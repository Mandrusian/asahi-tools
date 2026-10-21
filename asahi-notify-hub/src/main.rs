use std::process::Command;
use std::time::Duration;

use asahi_core::notify_config::NotifyConfig;
use asahi_core::{charge, find_fan_hwmon, get_battery_capacity, scan_fans, scan_temps};

const POLL_SECS: u64 = 30;
const HYSTERESIS: f64 = 3.0;

#[derive(Default)]
struct State {
    battery_target_triggered: bool,
    battery_low_triggered: bool,
    thermal_triggered: bool,
    fan_max_triggered: bool,
    override_was_active: bool,
    prev_cfg: Option<NotifyConfig>,
}

impl State {
    fn init(
        cfg: &NotifyConfig,
        cap: Option<u32>,
        target: u32,
        max_temp: f64,
        override_active: bool,
    ) -> Self {
        let mut s = Self::default();
        if let Some(c) = cap {
            s.battery_target_triggered = cfg.battery_target_reached.enabled && c >= target;
            s.battery_low_triggered =
                cfg.battery_low.enabled && (c as f64) <= cfg.battery_low.threshold;
        }
        s.thermal_triggered = cfg.thermal_high.enabled && max_temp >= cfg.thermal_high.threshold;
        s.override_was_active = override_active;
        s.prev_cfg = Some(cfg.clone());
        s
    }
}

fn notify(summary: &str, body: &str, icon: &str, urgency: &str) {
    let _ = Command::new("notify-send")
        .args(["-u", urgency, "-i", icon, summary, body])
        .status();
}

fn main() {
    eprintln!("asahi-notify-hub: starting");
    let mut cfg = NotifyConfig::load();
    let mut last_cfg_mtime = config_mtime();

    // Read initial state
    let cap = get_battery_capacity().ok();
    let charge_st = charge::status();
    let target = charge_st.as_ref().map(|c| c.effective_target).unwrap_or(80);
    let override_active = charge_st
        .as_ref()
        .map(|c| c.override_active)
        .unwrap_or(false);
    let (max_temp, _fan_peak) = read_thermal();

    let mut state = State::init(&cfg, cap, target, max_temp, override_active);
    eprintln!(
        "asahi-notify-hub: initialized (cap={:?}, target={}, max_temp={:.1}, override={})",
        cap, target, max_temp, override_active
    );

    loop {
        std::thread::sleep(Duration::from_secs(POLL_SECS));

        // Reload config if mtime changed
        let now_mtime = config_mtime();
        if now_mtime != last_cfg_mtime {
            cfg = NotifyConfig::load();
            last_cfg_mtime = now_mtime;
            eprintln!("asahi-notify-hub: config reloaded");
        }

        if !cfg.enabled {
            continue;
        }

        let cap = get_battery_capacity().ok();
        let charge_st = charge::status();
        let target = charge_st.as_ref().map(|c| c.effective_target).unwrap_or(80);
        let override_active = charge_st
            .as_ref()
            .map(|c| c.override_active)
            .unwrap_or(false);
        let (max_temp, fan_peak) = read_thermal();

        // ── battery target reached ─────────────────────────
        if let Some(c) = cap {
            let at = c >= target;
            if at && !state.battery_target_triggered && cfg.battery_target_reached.enabled {
                notify(
                    &format!("Battery at {}%", c),
                    &format!(
                        "Target {}% reached. Unplug for best battery health.",
                        target
                    ),
                    "battery-good",
                    "normal",
                );
                state.battery_target_triggered = true;
            } else if !at && (c as f64) < (target as f64) - HYSTERESIS {
                state.battery_target_triggered = false;
            }

            // ── battery low ────────────────────────────────
            let low = (c as f64) <= cfg.battery_low.threshold;
            if low && !state.battery_low_triggered && cfg.battery_low.enabled {
                notify(
                    &format!("Battery low: {}%", c),
                    &format!("Plug in soon (threshold {:.0}%)", cfg.battery_low.threshold),
                    "battery-caution",
                    "critical",
                );
                state.battery_low_triggered = true;
            } else if !low && (c as f64) > cfg.battery_low.threshold + HYSTERESIS {
                state.battery_low_triggered = false;
            }
        }

        // ── thermal high ────────────────────────────────────
        let hot = max_temp >= cfg.thermal_high.threshold;
        if hot && !state.thermal_triggered && cfg.thermal_high.enabled {
            notify(
                &format!("CPU at {:.1} °C", max_temp),
                &format!("Above {:.0} °C threshold", cfg.thermal_high.threshold),
                "dialog-warning",
                "normal",
            );
            state.thermal_triggered = true;
        } else if !hot && max_temp < cfg.thermal_high.threshold - 5.0 {
            state.thermal_triggered = false;
        }

        // ── fan ramped to max ───────────────────────────────
        if fan_peak > 0.0 {
            let at_max = fan_peak >= 0.92;
            if at_max && !state.fan_max_triggered && cfg.fan_ramped_max.enabled {
                notify(
                    "Fans at maximum",
                    "Thermal system is working hard — check load or reduce workload",
                    "fan",
                    "low",
                );
                state.fan_max_triggered = true;
            } else if !at_max && fan_peak < 0.80 {
                state.fan_max_triggered = false;
            }
        }

        // ── override expired ────────────────────────────────
        if state.override_was_active && !override_active && cfg.charge_override_expired.enabled {
            notify(
                "Charge override ended",
                "Returning to normal charge target",
                "battery-good",
                "low",
            );
        }
        state.override_was_active = override_active;

        state.prev_cfg = Some(cfg.clone());
    }
}

fn read_thermal() -> (f64, f64) {
    let hwmon = match find_fan_hwmon() {
        Some(h) => h,
        None => return (0.0, 0.0),
    };
    let max_t = scan_temps(&hwmon)
        .iter()
        .map(|t| t.celsius)
        .fold(0.0_f64, f64::max);
    let fans = scan_fans(&hwmon);
    // Peak fan as fraction of max — need to compute avg of "load"
    let peak = fans
        .iter()
        .map(|f| {
            let max = f.max.unwrap_or(6500) as f64;
            if max > 0.0 {
                f.rpm as f64 / max
            } else {
                0.0
            }
        })
        .fold(0.0_f64, f64::max);
    (max_t, peak)
}

fn config_mtime() -> Option<std::time::SystemTime> {
    std::fs::metadata(asahi_core::notify_config::config_path())
        .ok()
        .and_then(|m| m.modified().ok())
}
