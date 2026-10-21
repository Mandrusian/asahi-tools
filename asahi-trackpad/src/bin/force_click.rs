use std::thread;
use std::time::{Duration, Instant};

use asahi_core::trackpad_config::TrackpadConfig;
use evdev::uinput::VirtualDeviceBuilder;
use evdev::{
    AbsoluteAxisType, AttributeSet, Device, EventType, InputEvent, InputEventKind, Key,
    RelativeAxisType,
};

const CONFIG_POLL: Duration = Duration::from_secs(3);

fn find_trackpad() -> Option<Device> {
    for (_p, d) in evdev::enumerate() {
        if let Some(n) = d.name() {
            let l = n.to_lowercase();
            if l.contains("trackpad") && (l.contains("apple") || l.contains("spi")) {
                return Some(d);
            }
        }
    }
    None
}

fn read_pressure(ev: &evdev::InputEvent) -> Option<i32> {
    if let InputEventKind::AbsAxis(a) = ev.kind() {
        match a {
            AbsoluteAxisType::ABS_MT_PRESSURE | AbsoluteAxisType::ABS_PRESSURE => {
                return Some(ev.value());
            }
            _ => {}
        }
    }
    None
}

fn build_virtual_mouse() -> Result<evdev::uinput::VirtualDevice, Box<dyn std::error::Error>> {
    let mut keys = AttributeSet::<Key>::new();
    keys.insert(Key::BTN_LEFT);
    keys.insert(Key::BTN_RIGHT);
    let mut rels = AttributeSet::<RelativeAxisType>::new();
    rels.insert(RelativeAxisType::REL_X);
    rels.insert(RelativeAxisType::REL_Y);
    Ok(VirtualDeviceBuilder::new()?
        .name("asahi-force-click")
        .with_keys(&keys)?
        .with_relative_axes(&rels)?
        .build()?)
}

fn emit_right_click(virt: &mut evdev::uinput::VirtualDevice) -> std::io::Result<()> {
    let press = InputEvent::new(EventType::KEY, Key::BTN_RIGHT.0, 1);
    let sync1 = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
    virt.emit(&[press, sync1])?;
    thread::sleep(Duration::from_millis(10));
    let rel = InputEvent::new(EventType::KEY, Key::BTN_RIGHT.0, 0);
    let sync2 = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
    virt.emit(&[rel, sync2])?;
    Ok(())
}

fn config_mtime() -> Option<std::time::SystemTime> {
    std::fs::metadata(asahi_core::trackpad_config::config_path())
        .ok()
        .and_then(|m| m.modified().ok())
}

fn run_daemon() {
    let mut dev = match find_trackpad() {
        Some(d) => d,
        None => {
            eprintln!("force-click: no Apple trackpad");
            std::process::exit(1);
        }
    };
    if let Some(n) = dev.name() {
        eprintln!("force-click: opened {n}");
    }

    let mut cfg = TrackpadConfig::load();
    let mut last_mtime = config_mtime();

    if !cfg.is_calibrated() {
        eprintln!(
            "force-click: config is not calibrated; run `asahi-force-click --calibrate` first"
        );
        eprintln!(
            "force-click: using tentative defaults (hard={}, release={})",
            cfg.hard_press, cfg.release
        );
    } else {
        eprintln!(
            "force-click: calibrated  hard_press={}  release={}",
            cfg.hard_press, cfg.release
        );
    }
    eprintln!(
        "force-click: watching {} for changes",
        asahi_core::trackpad_config::config_path().display()
    );

    let mut virt = match build_virtual_mouse() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("force-click: uinput build failed: {e}");
            std::process::exit(1);
        }
    };

    let mut armed = true;
    let mut cur = 0i32;
    let mut last_cfg_check = Instant::now();

    loop {
        let events = match dev.fetch_events() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            }
        };
        for ev in events {
            if let Some(v) = read_pressure(&ev) {
                cur = v;
            }
        }

        if last_cfg_check.elapsed() > CONFIG_POLL {
            let now_mtime = config_mtime();
            if now_mtime != last_mtime {
                cfg = TrackpadConfig::load();
                last_mtime = now_mtime;
                eprintln!(
                    "force-click: config reloaded  hard_press={}  release={}  enabled={}",
                    cfg.hard_press, cfg.release, cfg.enabled
                );
            }
            last_cfg_check = Instant::now();
        }

        if !cfg.enabled {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        if armed && cur >= cfg.hard_press {
            match emit_right_click(&mut virt) {
                Ok(_) => eprintln!(
                    "force-click: RIGHT CLICK (p={cur}, threshold={})",
                    cfg.hard_press
                ),
                Err(e) => eprintln!("emit failed: {e}"),
            }
            armed = false;
        } else if !armed && cur < cfg.release {
            armed = true;
        }
        thread::sleep(Duration::from_millis(3));
    }
}

// ── Calibration ─────────────────────────────────────────────

const PRESS_START: i32 = 100; // pressure above this = "touch started"
const RELEASE_DETECT: i32 = 40; // pressure below this = "lifted"
const RELEASE_STABLE_MS: u128 = 150;
const TAPS_PER_PHASE: usize = 3;
const CYCLE_TIMEOUT: Duration = Duration::from_secs(15);

fn wait_for_press_cycle(dev: &mut Device, timeout: Duration) -> Option<i32> {
    let start = Instant::now();
    let mut peak: i32 = 0;
    let mut pressed = false;
    let mut below_since: Option<Instant> = None;

    while start.elapsed() < timeout {
        if let Ok(events) = dev.fetch_events() {
            for ev in events {
                let v = match read_pressure(&ev) {
                    Some(v) => v,
                    None => continue,
                };
                if !pressed && v > PRESS_START {
                    pressed = true;
                    peak = v;
                    below_since = None;
                } else if pressed {
                    if v > peak {
                        peak = v;
                    }
                    if v < RELEASE_DETECT {
                        match below_since {
                            None => below_since = Some(Instant::now()),
                            Some(t) if t.elapsed().as_millis() >= RELEASE_STABLE_MS => {
                                return Some(peak);
                            }
                            _ => {}
                        }
                    } else {
                        below_since = None;
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    if pressed {
        Some(peak)
    } else {
        None
    }
}

fn capture_phase(dev: &mut Device, label: &str, instruction: &str, n: usize) -> Vec<i32> {
    println!();
    println!("── {} ──", label);
    println!("  {}", instruction);
    println!();
    let mut peaks = Vec::new();
    for i in 1..=n {
        println!("  {}/{} — waiting for touch...", i, n);
        match wait_for_press_cycle(dev, CYCLE_TIMEOUT) {
            Some(peak) => {
                println!("        → peak: {}", peak);
                peaks.push(peak);
            }
            None => println!("        → timeout (skipped)"),
        }
    }
    peaks
}

fn run_calibration() {
    let mut dev = match find_trackpad() {
        Some(d) => d,
        None => {
            eprintln!("calibrate: no Apple trackpad");
            std::process::exit(1);
        }
    };
    if let Some(n) = dev.name() {
        eprintln!("calibrate: opened {n}");
    }

    println!("=== asahi-force-click calibration ===");
    println!();
    println!("This has two phases. Do a few taps/presses in each.");
    println!();
    println!("Press ENTER when ready to start...");
    {
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
    }

    let light_peaks = capture_phase(
        &mut dev,
        "Phase 1: LIGHT taps",
        "Do 3 taps like a normal left-click. One at a time.",
        TAPS_PER_PHASE,
    );
    if light_peaks.is_empty() {
        eprintln!("calibrate: no light taps detected; aborting");
        std::process::exit(1);
    }
    let light_max = *light_peaks.iter().max().unwrap();

    let hard_peaks = capture_phase(
        &mut dev,
        "Phase 2: HARD presses",
        "Do 3 presses through the force-click. One at a time.",
        TAPS_PER_PHASE,
    );
    if hard_peaks.is_empty() {
        eprintln!("calibrate: no hard presses detected; aborting");
        std::process::exit(1);
    }
    let hard_max = *hard_peaks.iter().max().unwrap();

    if hard_max <= light_max {
        eprintln!(
            "calibrate: hard max ({}) is not above light max ({}); \
             your panel may not report enough pressure range",
            hard_max, light_max
        );
        std::process::exit(1);
    }

    let span = hard_max - light_max;
    let hard_thresh = light_max + span / 2;
    let release_thresh = light_max + span / 4;

    let mut cfg = TrackpadConfig::load();
    cfg.enabled = true;
    cfg.hard_press = hard_thresh;
    cfg.release = release_thresh;
    cfg.observed_light_max = Some(light_max);
    cfg.observed_hard_max = Some(hard_max);
    cfg.calibrated_at = Some(chrono_lite_now());

    match cfg.save() {
        Ok(()) => {
            println!();
            println!("── Result ──");
            println!("  light max:    {}", light_max);
            println!("  hard max:     {}", hard_max);
            println!("  hard_press:   {}", hard_thresh);
            println!("  release:      {}", release_thresh);
            println!();
            println!(
                "Saved to {}",
                asahi_core::trackpad_config::config_path().display()
            );
            println!("The daemon will pick this up within 3 seconds.");
            println!();
            println!("Restart the daemon if it's not running, or wait for auto-reload.");
        }
        Err(e) => {
            eprintln!("calibrate: save failed: {e}");
            std::process::exit(1);
        }
    }
}

fn chrono_lite_now() -> String {
    std::process::Command::new("date")
        .arg("+%Y-%m-%dT%H:%M:%S%z")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--calibrate") {
        run_calibration();
    } else if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("asahi-force-click");
        println!();
        println!("  (no args)        run as daemon, watching config for changes");
        println!("  --calibrate      interactive calibration, auto-saves to config");
        println!("  --help           this message");
        println!();
        println!(
            "Config: {}",
            asahi_core::trackpad_config::config_path().display()
        );
    } else {
        run_daemon();
    }
}
