use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use asahi_core::{find_fan_hwmon, scan_fans, scan_temps, write_hwmon_raw, Fan};
use serde::{Deserialize, Serialize};

const SOCKET_PATH: &str = "/run/asahi-sysmon.sock";
const TEMP_OVERRIDE_C: f64 = 85.0;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Request {
    Status,
    SetFan { fan: u32, target: u32 },
    Max { fan: u32 },
    Auto { fan: u32 },
}

#[derive(Serialize, Debug, Clone)]
struct FanView {
    index: u32,
    rpm: u32,
    min: Option<u32>,
    max: Option<u32>,
    target: Option<u32>,
}

#[derive(Serialize, Debug)]
struct StatusView {
    ok: bool,
    readonly: bool,
    max_temp_c: f64,
    temps: Vec<(String, f64)>,
    fans: Vec<FanView>,
    message: Option<String>,
}

struct State {
    manual_targets: Vec<Option<u32>>,
    readonly: bool,
}

fn main() {
    let hwmon = match find_fan_hwmon() {
        Some(h) => h,
        None => {
            eprintln!("error: macsmc-hwmon not found. Is the module loaded?");
            std::process::exit(1);
        }
    };
    eprintln!("hwmon: {hwmon}");

    // Detect write capability by attempting to write back the current target.
    let readonly = detect_readonly(&hwmon);
    if readonly {
        eprintln!("note: fan_target is not writable; running in read-only mode");
        eprintln!("      (try `sudo modprobe -r macsmc_hwmon && sudo modprobe macsmc_hwmon fan_control=1`)");
    }

    if Path::new(SOCKET_PATH).exists() {
        let _ = std::fs::remove_file(SOCKET_PATH);
    }

    let state = Arc::new(Mutex::new(State {
        manual_targets: vec![None; 8],
        readonly,
    }));

    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {SOCKET_PATH}: {e} (are you root?)");
            std::process::exit(1);
        }
    };
    let _ = std::fs::set_permissions(SOCKET_PATH, std::fs::Permissions::from_mode(0o666));
    eprintln!("listening on {SOCKET_PATH}");

    // Safety loop: keep temps monitored even when no client is connected.
    {
        let hwmon = hwmon.clone();
        let state = state.clone();
        thread::spawn(move || safety_loop(&hwmon, state));
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let hwmon = hwmon.clone();
                let state = state.clone();
                thread::spawn(move || {
                    let _ = handle_client(stream, &hwmon, state);
                });
            }
            Err(e) => eprintln!("accept: {e}"),
        }
    }
}

fn detect_readonly(hwmon: &str) -> bool {
    let target_path = Path::new(hwmon).join("fan1_target");
    if !target_path.exists() {
        return true;
    }
    // Only test if we're root; otherwise we can't distinguish EPERM from read-only.
    if !is_root() {
        return false;
    }
    let original = match std::fs::read_to_string(&target_path) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return true,
    };
    if write_hwmon_raw(hwmon, "fan1_target", &original).is_err() {
        return true;
    }
    false
}

fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("Uid:").and_then(|rest| {
                    rest.split_whitespace()
                        .next()
                        .and_then(|v| v.parse::<u32>().ok())
                })
            })
        })
        .map(|uid| uid == 0)
        .unwrap_or(false)
}

fn handle_client(
    mut stream: UnixStream,
    hwmon: &str,
    state: Arc<Mutex<State>>,
) -> std::io::Result<()> {
    let reader = BufReader::new(stream.try_clone()?);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let _ = writeln!(
                    stream,
                    "{}",
                    serde_json::json!({"ok": false, "error": format!("{e}")})
                );
                continue;
            }
        };
        let resp = handle_request(&req, hwmon, &state);
        writeln!(stream, "{resp}")?;
    }
    Ok(())
}

fn handle_request(req: &Request, hwmon: &str, state: &Arc<Mutex<State>>) -> serde_json::Value {
    // If the hwmon path went stale (module reloaded), re-find it.
    let hwmon = if Path::new(hwmon).exists() {
        hwmon.to_string()
    } else {
        match find_fan_hwmon() {
            Some(h) => h,
            None => return serde_json::json!({"ok": false, "error": "hwmon not found"}),
        }
    };
    let hwmon = hwmon.as_str();
    match req {
        Request::Status => {
            let mut st = state.lock().unwrap();
            build_status(hwmon, &mut st)
        }
        Request::SetFan { fan, target } => {
            let mut st = state.lock().unwrap();
            if st.readonly {
                return serde_json::json!({"ok": false, "error": "fan control is read-only"});
            }
            let fans = scan_fans(hwmon);
            let f = match fans.iter().find(|f| f.index == *fan) {
                Some(f) => f.clone(),
                None => return serde_json::json!({"ok": false, "error": "no such fan"}),
            };
            let max_allowed = f.max.unwrap_or(6500);
            let min_allowed = f.min.unwrap_or(1200);
            let clamped = (*target).clamp(min_allowed, max_allowed);
            if let Err(e) =
                write_hwmon_raw(hwmon, &format!("fan{fan}_target"), &clamped.to_string())
            {
                return serde_json::json!({"ok": false, "error": format!("{e}")});
            }
            if let Some(slot) = st.manual_targets.get_mut(*fan as usize) {
                *slot = Some(clamped);
            }
            serde_json::json!({"ok": true, "target": clamped})
        }
        Request::Max { fan } => {
            let fans = scan_fans(hwmon);
            let f = match fans.iter().find(|f| f.index == *fan) {
                Some(f) => f.clone(),
                None => return serde_json::json!({"ok": false, "error": "no such fan"}),
            };
            let max_allowed = f.max.unwrap_or(6500);
            handle_request(
                &Request::SetFan {
                    fan: *fan,
                    target: max_allowed,
                },
                hwmon,
                state,
            )
        }
        Request::Auto { fan: _ } => {
            {
                let mut st = state.lock().unwrap();
                st.manual_targets = vec![None; 8];
            }
            let _ = std::process::Command::new("modprobe")
                .args(["-r", "macsmc_hwmon"]).status();
            std::thread::sleep(std::time::Duration::from_millis(300));
            match std::process::Command::new("modprobe")
                .args(["macsmc_hwmon", "fan_control=1"]).status()
            {
                Ok(s) if s.success() => serde_json::json!({"ok": true, "note": "manual control released"}),
                _ => serde_json::json!({"ok": false, "error": "module reload failed"}),
            }
        }
    }
}

fn build_status(hwmon: &str, st: &mut State) -> serde_json::Value {
    let temps = scan_temps(hwmon);
    let fans = scan_fans(hwmon);
    let max_temp = temps.iter().map(|t| t.celsius).fold(0.0_f64, f64::max);

    let fan_views: Vec<FanView> = fans
        .iter()
        .map(|f: &Fan| FanView {
            index: f.index,
            rpm: f.rpm,
            min: f.min,
            max: f.max,
            target: st.manual_targets.get(f.index as usize).copied().flatten(),
        })
        .collect();

    let view = StatusView {
        ok: true,
        readonly: st.readonly,
        max_temp_c: max_temp,
        temps: temps.iter().map(|t| (t.name.clone(), t.celsius)).collect(),
        fans: fan_views,
        message: None,
    };

    serde_json::to_value(view)
        .unwrap_or_else(|e| serde_json::json!({"ok": false, "error": format!("{e}")}))
}

fn safety_loop(hwmon: &str, state: Arc<Mutex<State>>) {
    loop {
        let temps = scan_temps(hwmon);
        let max_temp = temps.iter().map(|t| t.celsius).fold(0.0_f64, f64::max);
        let fans = scan_fans(hwmon);
        {
            let st = state.lock().unwrap();
            if !st.readonly && max_temp > TEMP_OVERRIDE_C {
                for f in &fans {
                    let max_allowed = f.max.unwrap_or(6500);
                    let _ = write_hwmon_raw(
                        hwmon,
                        &format!("fan{}_target", f.index),
                        &max_allowed.to_string(),
                    );
                }
            }
        }
        thread::sleep(std::time::Duration::from_secs(2));
    }
}
