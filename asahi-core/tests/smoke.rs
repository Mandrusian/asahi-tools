// Smoke tests — catch regressions in paths, defaults, and public API.
// These don't need hardware; they test what can go wrong in refactors.

use asahi_core::{charge, notify_config, priv_daemon, sysmon, trackpad_config};

#[test]
fn trackpad_config_path_under_home() {
    let home = std::env::var_os("HOME").expect("HOME must be set");
    let path = trackpad_config::config_path();
    assert!(
        path.starts_with(&home),
        "config_path {:?} must be under HOME {:?}",
        path,
        home
    );
    assert!(path.ends_with("trackpad.json"));
}

#[test]
fn trackpad_defaults_are_sane() {
    let c = trackpad_config::TrackpadConfig::default();
    assert!(c.enabled, "should default to enabled");
    assert!(
        c.hard_press > c.release,
        "hard_press ({}) must be > release ({})",
        c.hard_press,
        c.release
    );
    assert!(c.hard_press > 0 && c.hard_press < 10000);
}

#[test]
fn trackpad_config_roundtrip() {
    let original = trackpad_config::TrackpadConfig {
        enabled: false,
        hard_press: 999,
        release: 555,
        observed_light_max: Some(200),
        observed_hard_max: Some(1000),
        calibrated_at: Some("test".into()),
    };
    let json = serde_json::to_string(&original).unwrap();
    let parsed: trackpad_config::TrackpadConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.hard_press, 999);
    assert_eq!(parsed.release, 555);
    assert!(!parsed.enabled);
}

#[test]
fn notify_config_path_under_home() {
    let home = std::env::var_os("HOME").expect("HOME must be set");
    let path = notify_config::config_path();
    assert!(
        path.starts_with(&home),
        "notify config {:?} must be under HOME {:?}",
        path,
        home
    );
    assert!(path.ends_with("notifications.json"));
}

#[test]
fn notify_defaults_enable_useful_alerts() {
    let c = notify_config::NotifyConfig::default();
    assert!(c.enabled);
    assert!(c.battery_target_reached.enabled);
    assert!(c.battery_low.enabled);
    assert!(c.thermal_high.enabled);
    assert!(c.battery_low.threshold > 0.0 && c.battery_low.threshold < 100.0);
}

#[test]
fn socket_paths_match_daemons() {
    assert_eq!(priv_daemon::SOCKET, "/run/asahi-priv.sock");
    assert_eq!(charge::SOCKET, "/run/asahi-charge.sock");
    assert_eq!(sysmon::SOCKET, "/run/asahi-sysmon.sock");
}

#[test]
fn sysfs_reads_do_not_panic() {
    // These may return None/Err on non-Asahi systems — but must not panic.
    let _ = asahi_core::find_fan_hwmon();
    let _ = asahi_core::get_battery_capacity();
    let _ = asahi_core::get_battery_status();
    let _ = asahi_core::get_kbd_brightness();
    let _ = asahi_core::get_kbd_max_brightness();
    let _ = asahi_core::get_charge_thresholds();
}

#[test]
fn priv_daemon_not_running_is_clean() {
    // is_running() returns bool without panicking whether socket exists or not.
    let running = priv_daemon::is_running();
    // No assertion on value — just that it returns.
    let _ = running;
}

#[test]
fn hwmon_scan_and_rw_on_temp_dir() {
    let dir = std::env::temp_dir().join(format!("asahi_core_hwmon_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("temp2_input"), "45250\n").unwrap();
    std::fs::write(dir.join("temp1_input"), "38000\n").unwrap();
    std::fs::write(dir.join("temp3_input"), "not_a_number\n").unwrap();
    std::fs::write(dir.join("fan1_input"), "2400\n").unwrap();
    std::fs::write(dir.join("fan1_min"), "1200\n").unwrap();
    std::fs::write(dir.join("fan1_max"), "6500\n").unwrap();
    std::fs::write(dir.join("fan1_target"), "2500\n").unwrap();

    let dir_str = dir.to_str().unwrap();
    let temps = asahi_core::scan_temps(dir_str);
    assert_eq!(temps.len(), 2);
    assert_eq!(temps[0].name, "temp1");
    assert!((temps[0].celsius - 38.0).abs() < 1e-6);
    assert_eq!(temps[1].name, "temp2");
    assert!((temps[1].celsius - 45.25).abs() < 1e-6);

    let fans = asahi_core::scan_fans(dir_str);
    assert_eq!(fans.len(), 1);
    assert_eq!(fans[0].index, 1);
    assert_eq!(fans[0].rpm, 2400);
    assert_eq!(fans[0].min, Some(1200));
    assert_eq!(fans[0].max, Some(6500));
    assert_eq!(fans[0].target, Some(2500));

    asahi_core::write_hwmon_raw(dir_str, "fan1_target", "3000").unwrap();
    assert_eq!(
        asahi_core::read_hwmon_i64(dir_str, "fan1_target").unwrap(),
        3000
    );
    assert!((asahi_core::read_hwmon_f64(dir_str, "fan1_target").unwrap() - 3000.0).abs() < 1e-6);

    assert!(matches!(
        asahi_core::read_hwmon_raw(dir_str, "missing"),
        Err(asahi_core::AsahiError::PathNotFound(_))
    ));
    assert!(matches!(
        asahi_core::read_hwmon_i64(dir_str, "temp3_input"),
        Err(asahi_core::AsahiError::InvalidValue(_))
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn charge_fmt_duration_formats_cleanly() {
    assert_eq!(charge::fmt_duration(45), "45s");
    assert_eq!(charge::fmt_duration(65), "1m 05s");
    assert_eq!(charge::fmt_duration(3665), "1h 01m");
}

#[test]
fn notify_config_helpers_and_partial_deserialization() {
    let mut t = notify_config::Toggle::off();
    assert!(!t.enabled);
    t.flip();
    assert!(t.enabled);

    let mut th = notify_config::Threshold::new(20.0);
    th.bump(-50.0, 5.0, 95.0);
    assert_eq!(th.threshold, 5.0);
    th.bump(100.0, 5.0, 95.0);
    assert_eq!(th.threshold, 95.0);

    let partial: notify_config::NotifyConfig =
        serde_json::from_str(r#"{"enabled":false}"#).unwrap();
    assert!(!partial.enabled);
    assert!(partial.battery_low.enabled);
    assert_eq!(partial.battery_low.threshold, 20.0);
}

#[test]
fn trackpad_is_calibrated_requires_all_fields() {
    let mut cfg = trackpad_config::TrackpadConfig::default();
    assert!(!cfg.is_calibrated());
    cfg.observed_light_max = Some(150);
    cfg.observed_hard_max = Some(900);
    assert!(!cfg.is_calibrated());
    cfg.calibrated_at = Some("2026-01-01T00:00:00Z".into());
    assert!(cfg.is_calibrated());
}

#[test]
fn daemon_request_serialization_tags() {
    let priv_req = serde_json::to_value(priv_daemon::Request::SetProfile {
        profile: "balanced".into(),
    })
    .unwrap();
    assert_eq!(priv_req["cmd"], "set_profile");
    assert_eq!(priv_req["profile"], "balanced");

    let charge_req = serde_json::to_value(charge::Request::Override {
        percent: 100,
        duration_secs: 7200,
    })
    .unwrap();
    assert_eq!(charge_req["cmd"], "override");
    assert_eq!(charge_req["percent"], 100);

    let sys_req = serde_json::to_value(sysmon::Request::SetFan {
        fan: 1,
        target: 3200,
    })
    .unwrap();
    assert_eq!(sys_req["cmd"], "set_fan");
    assert_eq!(sys_req["fan"], 1);
    assert_eq!(sys_req["target"], 3200);
}
