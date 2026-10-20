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
