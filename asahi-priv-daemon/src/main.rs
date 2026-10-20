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

    for stream in listener.incoming() {
        if let Ok(s) = stream {
            thread::spawn(move || { let _ = handle_client(s); });
        }
    }
}
