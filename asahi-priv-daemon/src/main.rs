use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

const SOCKET: &str = "/run/asahi-priv.sock";
const KBD: &str = "/sys/class/leds/kbd_backlight/brightness";
const KBD_MAX: &str = "/sys/class/leds/kbd_backlight/max_brightness";
const PROFILE: &str = "/sys/firmware/acpi/platform_profile";
const FNMODE: &str = "/sys/module/hid_apple/parameters/fnmode";

const DISP_PATHS: &[&str] = &[
    "/sys/class/backlight/apple-panel-bl/brightness",
    "/sys/class/backlight/apple-panel-backlight/brightness",
    "/sys/class/backlight/apple_dcp_bl/brightness",
    "/sys/class/backlight/dcp-backlight/brightness",
    "/sys/class/backlight/edp-backlight/brightness",
    "/sys/class/backlight/panel0-backlight/brightness",
];

fn find_disp() -> Option<&'static str> {
    DISP_PATHS.iter().copied().find(|p| std::path::Path::new(p).exists())
}

fn disp_max_path() -> Option<String> {
    find_disp().map(|p| p.replace("brightness", "max_brightness"))
}

const ALLOWED_MODULES: &[&str] = &[
    "brcmfmac", "hci_bcm4377", "macsmc_hwmon",
    "apple_dcp", "applespi", "hid_apple", "snd_soc_macaudio",
];

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Request {
    Status,
    GetKbd,
    SetKbd { value: u32 },
    GetDisplay,
    SetDisplay { value: u32 },
    GetProfile,
    SetProfile { profile: String },
    RestartModule { module: String },
    GetFnMode,
    SetFnMode { value: u32 },
    ServiceAction { name: String, action: String },
    ServiceStatus { name: String },
    AlsStatus,
    AlsSetEnabled { enabled: bool },
    AlsManualOverride,
}

const ALLOWED_SERVICES: &[&str] = &[
    "asahi-charge-daemon",
    "asahi-sysmon-daemon",
    "asahi-priv-daemon",
    "asahi-force-click",
];

const ALLOWED_ACTIONS: &[&str] = &["start", "stop", "restart", "enable", "disable"];

fn service_status(name: &str) -> serde_json::Value {
    let active = Command::new("systemctl")
        .args(["is-active", name])
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let enabled = Command::new("systemctl")
        .args(["is-enabled", name])
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let detail = Command::new("systemctl")
        .args(["show", name, "--property=ActiveState,SubState"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    serde_json::json!({
        "ok": true,
        "active": active,
        "enabled": enabled,
        "detail": detail,
    })
}

fn read_u32(path: &str) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn write_val(path: &str, val: &str) -> Result<(), String> {
    std::fs::write(path, val).map_err(|e| e.to_string())
}

fn handle(req: Request) -> serde_json::Value {
    match req {
        Request::GetKbd => serde_json::json!({
            "ok": true,
            "current": read_u32(KBD),
            "max": read_u32(KBD_MAX).unwrap_or(255),
        }),
        Request::SetKbd { value } => {
            let max = read_u32(KBD_MAX).unwrap_or(255);
            let v = value.min(max);
            match write_val(KBD, &v.to_string()) {
                Ok(_) => serde_json::json!({"ok": true, "value": v}),
                Err(e) => serde_json::json!({"ok": false, "error": e}),
            }
        }
        Request::GetDisplay => {
            let path = find_disp();
            let max = disp_max_path().and_then(|p| read_u32(&p)).unwrap_or(100);
            serde_json::json!({
                "ok": path.is_some(),
                "current": path.and_then(read_u32),
                "max": max,
                "path": path,
                "error": if path.is_none() { Some("no backlight device") } else { None },
            })
        }
        Request::SetDisplay { value } => {
            let path = match find_disp() {
                Some(p) => p,
                None => return serde_json::json!({"ok": false, "error": "no backlight device"}),
            };
            let max = disp_max_path().and_then(|p| read_u32(&p)).unwrap_or(100);
            let v = value.min(max);
            match write_val(path, &v.to_string()) {
                Ok(_) => serde_json::json!({"ok": true, "value": v, "path": path}),
                Err(e) => serde_json::json!({"ok": false, "error": e}),
            }
        }
        Request::GetProfile => {
            let p = std::fs::read_to_string(PROFILE).ok()
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "unknown".into());
            serde_json::json!({"ok": true, "profile": p})
        }
        Request::SetProfile { profile } => {
            if !["quiet", "balanced", "performance"].contains(&profile.as_str()) {
                return serde_json::json!({"ok": false, "error": "invalid profile"});
            }
            match write_val(PROFILE, &profile) {
                Ok(_) => serde_json::json!({"ok": true, "profile": profile}),
                Err(e) => serde_json::json!({"ok": false, "error": e}),
            }
        }
        Request::RestartModule { module } => {
            if !ALLOWED_MODULES.contains(&module.as_str()) {
                return serde_json::json!({"ok": false, "error": "module not whitelisted"});
            }
            let _ = Command::new("modprobe").args(["-r", &module]).status();
            thread::sleep(Duration::from_millis(400));
            match Command::new("modprobe").arg(&module).status() {
                Ok(s) if s.success() => serde_json::json!({"ok": true, "module": module}),
                _ => serde_json::json!({"ok": false, "error": "reload failed"}),
            }
        }
        Request::GetFnMode => serde_json::json!({
            "ok": true,
            "fnmode": read_u32(FNMODE),
        }),
        Request::SetFnMode { value } => {
            if value > 3 {
                return serde_json::json!({"ok": false, "error": "fnmode must be 0-3"});
            }
            match write_val(FNMODE, &value.to_string()) {
                Ok(_) => serde_json::json!({"ok": true, "fnmode": value}),
                Err(e) => serde_json::json!({"ok": false, "error": e}),
            }
        }
        Request::Status => serde_json::json!({
            "ok": true,
            "kbd": read_u32(KBD),
            "kbd_max": read_u32(KBD_MAX),
            "display": find_disp().and_then(read_u32),
            "display_max": disp_max_path().and_then(|p| read_u32(&p)),
            "profile": std::fs::read_to_string(PROFILE).ok().map(|s| s.trim().to_string()),
            "fnmode": read_u32(FNMODE),
        }),
        Request::ServiceStatus { name } => {
            if !ALLOWED_SERVICES.contains(&name.as_str()) {
                return serde_json::json!({"ok": false, "error": "service not whitelisted"});
            }
            service_status(&name)
        }
        Request::ServiceAction { name, action } => {
            if !ALLOWED_SERVICES.contains(&name.as_str()) {
                return serde_json::json!({"ok": false, "error": "service not whitelisted"});
            }
            if !ALLOWED_ACTIONS.contains(&action.as_str()) {
                return serde_json::json!({"ok": false, "error": "action not whitelisted"});
            }
            let status = Command::new("systemctl")
                .arg(&action)
                .arg(&name)
                .status();
            match status {
                Ok(s) if s.success() => serde_json::json!({"ok": true, "action": action, "name": name}),
                Ok(s) => serde_json::json!({"ok": false, "error": format!("exit {}", s.code().unwrap_or(-1))}),
                Err(e) => serde_json::json!({"ok": false, "error": e.to_string()}),
            }
        }
        Request::AlsStatus => {
            let cfg = load_als_config();
            let lux = std::fs::read_to_string(&cfg.sensor_path)
                .ok()
                .and_then(|s| s.trim().parse::<f64>().ok());
            let now = now_unix();
            let until = ALS_OVERRIDE_UNTIL.load(Ordering::Relaxed);
            serde_json::json!({
                "ok": true,
                "enabled": ALS_ENABLED.load(Ordering::Relaxed),
                "lux": lux,
                "override_active": now < until,
                "override_remaining_secs": (until - now).max(0),
                "sensor_path": cfg.sensor_path,
                "sensor_available": std::path::Path::new(&cfg.sensor_path).exists(),
            })
        }
        Request::AlsSetEnabled { enabled } => {
            ALS_ENABLED.store(enabled, Ordering::Relaxed);
            let mut cfg = load_als_config();
            cfg.enabled = enabled;
            let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
            let path = home.join(".config/asahi-tools/als.json");
            let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
            let _ = std::fs::write(&path, serde_json::to_string_pretty(&serde_json::json!({
                "enabled": cfg.enabled,
                "sensor_path": cfg.sensor_path,
                "curve": cfg.curve,
                "smoothing_ms": cfg.smoothing_ms,
                "manual_override_secs": cfg.manual_override_secs,
            })).unwrap_or_default());
            serde_json::json!({"ok": true, "enabled": enabled})
        }
        Request::AlsManualOverride => {
            let cfg = load_als_config();
            let until = now_unix() + cfg.manual_override_secs;
            ALS_OVERRIDE_UNTIL.store(until, Ordering::Relaxed);
            serde_json::json!({"ok": true, "override_secs": cfg.manual_override_secs})
        }
    }
}

fn handle_client(stream: UnixStream) -> std::io::Result<()> {
    let mut writer = stream.try_clone()?;
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let _ = writeln!(writer, "{{\"ok\":false,\"error\":\"{e}\"}}");
                continue;
            }
        };
        let resp = handle(req);
        writeln!(writer, "{resp}")?;
    }
    Ok(())
}

// ── ALS (ambient light sensor) ──────────────────────────
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;

static ALS_ENABLED: AtomicBool = AtomicBool::new(false);
static ALS_OVERRIDE_UNTIL: AtomicI64 = AtomicI64::new(0);

#[derive(serde::Deserialize, Clone)]
struct AlsCurvePoint {
    lux: f64,
    brightness: f64,
}

#[derive(serde::Deserialize, Clone)]
struct AlsConfig {
    enabled: bool,
    sensor_path: String,
    curve: Vec<(f64, f64)>,
    smoothing_ms: u64,
    manual_override_secs: i64,
}

impl Default for AlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sensor_path: "/sys/bus/iio/devices/iio:device0/in_illuminance_input".into(),
            curve: vec![(0.0, 10.0), (50.0, 30.0), (200.0, 55.0), (1000.0, 85.0), (5000.0, 100.0)],
            smoothing_ms: 500,
            manual_override_secs: 300,
        }
    }
}

fn load_als_config() -> AlsConfig {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let p = home.join(".config/asahi-tools/als.json");
    match std::fs::read_to_string(&p) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => AlsConfig::default(),
    }
}

fn interpolate_curve(curve: &[(f64, f64)], lux: f64) -> f64 {
    if curve.is_empty() { return 50.0; }
    if lux <= curve[0].0 { return curve[0].1; }
    if lux >= curve[curve.len() - 1].0 { return curve[curve.len() - 1].1; }
    for i in 1..curve.len() {
        if lux <= curve[i].0 {
            let (x0, y0) = curve[i - 1];
            let (x1, y1) = curve[i];
            let t = (lux - x0) / (x1 - x0);
            return y0 + t * (y1 - y0);
        }
    }
    curve[curve.len() - 1].1
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn als_loop(cfg: AlsConfig) {
    eprintln!("als: thread started (sensor={}, curve points={})", cfg.sensor_path, cfg.curve.len());
    ALS_ENABLED.store(cfg.enabled, Ordering::Relaxed);
    let mut last_brightness = -1.0f64;
    loop {
        std::thread::sleep(Duration::from_millis(cfg.smoothing_ms));
        if !ALS_ENABLED.load(Ordering::Relaxed) { continue; }
        // Check manual override window
        let now = now_unix();
        let until = ALS_OVERRIDE_UNTIL.load(Ordering::Relaxed);
        if now < until { continue; }
        // Read lux
        let lux = match std::fs::read_to_string(&cfg.sensor_path) {
            Ok(s) => match s.trim().parse::<f64>() { Ok(v) => v, Err(_) => continue },
            Err(_) => continue,
        };
        // Compute target brightness (as percent of max)
        let target_pct = interpolate_curve(&cfg.curve, lux);
        // Smooth towards target
        let target = if last_brightness < 0.0 { target_pct } else {
            last_brightness + (target_pct - last_brightness) * 0.25
        };
        last_brightness = target;
        // Write to backlight
        if let Some(path) = find_disp() {
            let max = disp_max_path().and_then(|p| read_u32(&p)).unwrap_or(100);
            let val = ((target / 100.0) * max as f64).clamp(1.0, max as f64) as u32;
            let _ = write_val(path, &val.to_string());
        }
    }
}

fn main() {
    if Path::new(SOCKET).exists() {
        let _ = std::fs::remove_file(SOCKET);
    }
    let listener = match UnixListener::bind(SOCKET) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {SOCKET}: {e}");
            std::process::exit(1);
        }
    };
    // 0666 = any user can connect. Fine for a personal machine; the
    // operation whitelist is what enforces safety, not the socket mode.
    let _ = std::fs::set_permissions(SOCKET, std::fs::Permissions::from_mode(0o666));
    eprintln!("asahi-priv-daemon: listening on {SOCKET}");

    // Spawn ALS thread
    let als_cfg = load_als_config();
    let als_cfg_clone = als_cfg.clone();
    thread::spawn(move || als_loop(als_cfg_clone));

    for stream in listener.incoming() {
        if let Ok(s) = stream {
            thread::spawn(move || { let _ = handle_client(s); });
        }
    }
}
