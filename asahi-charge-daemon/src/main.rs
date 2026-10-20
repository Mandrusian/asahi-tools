use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const BAT_DIR: &str = "/sys/class/power_supply/macsmc-battery";
const SOCKET: &str = "/run/asahi-charge.sock";
const CONFIG: &str = "/etc/asahi-tools/charge-daemon.conf";

#[derive(Clone, Debug)]
struct Config {
    target: u32,
    hysteresis: u32,
    max_temp_c: u32,
    poll_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self { target: 80, hysteresis: 2, max_temp_c: 45, poll_secs: 30 }
    }
}

fn load_config() -> Config {
    let text = match std::fs::read_to_string(CONFIG) {
        Ok(t) => t,
        Err(_) => return Config::default(),
    };
    let mut cfg = Config::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim();
            match k.trim() {
                "target" => cfg.target = v.parse().unwrap_or(cfg.target),
                "hysteresis" => cfg.hysteresis = v.parse().unwrap_or(cfg.hysteresis),
                "max_temp_c" => cfg.max_temp_c = v.parse().unwrap_or(cfg.max_temp_c),
                "poll_secs" => cfg.poll_secs = v.parse().unwrap_or(cfg.poll_secs),
                _ => {}
            }
        }
    }
    cfg
}

fn save_config(cfg: &Config) -> std::io::Result<()> {
    if let Some(parent) = Path::new(CONFIG).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = format!(
        "# asahi-charge-daemon config\n# target: charge stops at this %\n\
         # hysteresis: charging resumes at target-hysteresis%\n\
         # max_temp_c: pause charging above this temperature\n\
         target={}\nhysteresis={}\nmax_temp_c={}\npoll_secs={}\n",
        cfg.target, cfg.hysteresis, cfg.max_temp_c, cfg.poll_secs
    );
    std::fs::write(CONFIG, text)
}

fn read_attr(name: &str) -> Option<String> {
    std::fs::read_to_string(format!("{BAT_DIR}/{name}"))
        .ok()
        .map(|s| s.trim().to_string())
}

fn read_capacity() -> Option<u32> {
    read_attr("capacity").and_then(|s| s.parse().ok())
}

fn read_temp_c() -> Option<f64> {
    read_attr("temp")
        .and_then(|s| s.parse::<f64>().ok())
        .map(|t| t / 10.0)
}

fn write_attr(name: &str, value: &str) -> Result<(), String> {
    std::fs::write(format!("{BAT_DIR}/{name}"), value).map_err(|e| e.to_string())
}

fn set_charging(cfg: &Config, enable: bool) -> Result<(), String> {
    if enable {
        let _ = std::fs::write(format!("{BAT_DIR}/charge_behaviour"), "auto");
        let start = cfg.target.saturating_sub(cfg.hysteresis);
        write_attr("charge_control_start_threshold", &start.to_string())?;
        write_attr("charge_control_end_threshold", &cfg.target.to_string())?;
    } else {
        if std::fs::write(format!("{BAT_DIR}/charge_behaviour"), "inhibit-charge").is_ok() {
            return Ok(());
        }
        let cap = read_capacity().unwrap_or(50);
        let stop_at = ((cap / 5) * 5).saturating_sub(5).max(50);
        write_attr("charge_control_start_threshold", &stop_at.to_string())?;
        write_attr("charge_control_end_threshold", &stop_at.to_string())?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Request {
    Status,
    Override { percent: u32, duration_secs: u64 },
    CancelOverride,
    SetTarget { target: u32, hysteresis: Option<u32> },
    Reload,
}

#[derive(Serialize, Debug, Clone)]
struct StatusView {
    ok: bool,
    capacity: Option<u32>,
    temp_c: Option<f64>,
    charging_enabled: bool,
    override_active: bool,
    override_remaining_secs: Option<u64>,
    effective_target: u32,
    config_target: u32,
    hysteresis: u32,
    max_temp_c: u32,
    last_action: String,
}

struct State {
    cfg: Config,
    override_until: Option<Instant>,
    override_percent: Option<u32>,
    charging_enabled: bool,
    last_capacity: Option<u32>,
    last_temp: Option<f64>,
    last_action: String,
}

fn parse_duration(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }
    if s.is_empty() {
        return None;
    }
    let (num, suffix) = s.split_at(s.len() - 1);
    let n: u64 = num.parse().ok()?;
    match suffix {
        "s" => Some(n),
        "m" => Some(n * 60),
        "h" => Some(n * 3600),
        "d" => Some(n * 86400),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        run_daemon();
        return;
    }

    match args[0].as_str() {
        "status" => cli_send(&Request::Status),
        "override" => {
            if args.len() < 3 {
                eprintln!("usage: asahi-charge-daemon override <percent> <duration>  e.g. override 100 2h");
                std::process::exit(1);
            }
            let percent: u32 = match args[1].parse() {
                Ok(p) => p,
                Err(_) => {
                    eprintln!("percent must be a number");
                    std::process::exit(1);
                }
            };
            let dur = match parse_duration(&args[2]) {
                Some(d) => d,
                None => {
                    eprintln!("duration must be like 30m, 2h, 86400s");
                    std::process::exit(1);
                }
            };
            cli_send(&Request::Override { percent, duration_secs: dur });
        }
        "cancel" => cli_send(&Request::CancelOverride),
        "set-target" => {
            if args.len() < 2 {
                eprintln!("usage: asahi-charge-daemon set-target <percent> [hysteresis]");
                std::process::exit(1);
            }
            let target: u32 = match args[1].parse() {
                Ok(t) => t,
                Err(_) => {
                    eprintln!("target must be a number");
                    std::process::exit(1);
                }
            };
            let hysteresis = args.get(2).and_then(|s| s.parse().ok());
            cli_send(&Request::SetTarget { target, hysteresis });
        }
        "reload" => cli_send(&Request::Reload),
        "help" | "-h" | "--help" => {
            println!("asahi-charge-daemon — active battery charge controller");
            println!();
            println!("  (no args)           run daemon");
            println!("  status              show current state");
            println!("  override P D        charge to P% for duration D (30m, 2h, 3600s)");
            println!("  cancel              cancel active override");
            println!("  set-target T [H]    set target T% and optional hysteresis H%");
            println!("  reload              reload /etc/asahi-tools/charge-daemon.conf");
            std::process::exit(0);
        }
        other => {
            eprintln!("unknown command: {other} (try 'help')");
            std::process::exit(1);
        }
    }
}

fn cli_send(req: &Request) -> ! {
    match send_request(req) {
        Ok(r) => {
            println!("{r}");
            std::process::exit(0)
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1)
        }
    }
}

fn send_request(req: &Request) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(SOCKET)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    writeln!(stream, "{}", serde_json::to_string(req).unwrap())?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn run_daemon() {
    let cfg = load_config();
    eprintln!(
        "asahi-charge-daemon: target={}% hysteresis={}% max_temp={}C poll={}s",
        cfg.target, cfg.hysteresis, cfg.max_temp_c, cfg.poll_secs
    );

    if Path::new(SOCKET).exists() {
        let _ = std::fs::remove_file(SOCKET);
    }

    let state = Arc::new(Mutex::new(State {
        cfg,
        override_until: None,
        override_percent: None,
        charging_enabled: true,
        last_capacity: None,
        last_temp: None,
        last_action: "started".into(),
    }));

    let listener = match UnixListener::bind(SOCKET) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {SOCKET}: {e} (are you root?)");
            std::process::exit(1);
        }
    };
    let _ = std::fs::set_permissions(SOCKET, std::fs::Permissions::from_mode(0o666));
    eprintln!("listening on {SOCKET}");

    {
        let state = state.clone();
        thread::spawn(move || control_loop(state));
    }

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            let state = state.clone();
            thread::spawn(move || {
                let _ = handle_client(stream, state);
            });
        }
    }
}

fn control_loop(state: Arc<Mutex<State>>) {
    loop {
        let sleep_secs = {
            let mut st = state.lock().unwrap();
            if let Err(e) = tick(&mut st) {
                st.last_action = format!("tick error: {e}");
            }
            st.cfg.poll_secs
        };
        thread::sleep(Duration::from_secs(sleep_secs.max(5)));
    }
}

fn tick(st: &mut State) -> Result<(), String> {
    let cap = read_capacity();
    let temp = read_temp_c();
    st.last_capacity = cap;
    st.last_temp = temp;

    if let Some(until) = st.override_until {
        if Instant::now() >= until {
            st.override_until = None;
            st.override_percent = None;
            st.last_action = "override expired".into();
        }
    }

    let too_hot = temp.map(|t| t > st.cfg.max_temp_c as f64).unwrap_or(false);
    let effective_target = st.override_percent.unwrap_or(st.cfg.target);

    // Determine desired state
    let want_enabled = if too_hot {
        false
    } else {
        match cap {
            Some(c) if c >= effective_target => false,
            _ => true,
        }
    };

    if want_enabled != st.charging_enabled {
        set_charging(&st.cfg, want_enabled)?;
        st.charging_enabled = want_enabled;
        st.last_action = if want_enabled {
            format!("charging enabled (cap={:?}, target={}%)", cap, effective_target)
        } else if too_hot {
            format!("paused (temp {:.1}C > {}C)", temp.unwrap_or(0.0), st.cfg.max_temp_c)
        } else {
            format!("paused at {}% (target {}%)", cap.unwrap_or(0), effective_target)
        };
    } else if st.last_action.is_empty() {
        st.last_action = format!("steady (cap={:?}, temp={:?})", cap, temp);
    }
    Ok(())
}

fn handle_client(stream: UnixStream, state: Arc<Mutex<State>>) -> std::io::Result<()> {
    let mut writer = stream.try_clone()?;
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let _ = writeln!(writer, "{{\"ok\":false,\"error\":\"{e}\"}}");
                continue;
            }
        };
        let resp = handle_request(req, &state);
        writeln!(writer, "{resp}")?;
    }
    Ok(())
}

fn handle_request(req: Request, state: &Arc<Mutex<State>>) -> serde_json::Value {
    let mut st = state.lock().unwrap();
    match req {
        Request::Status => {
            let remaining = st.override_until.map(|i| {
                i.saturating_duration_since(Instant::now()).as_secs()
            });
            let view = StatusView {
                ok: true,
                capacity: st.last_capacity,
                temp_c: st.last_temp,
                charging_enabled: st.charging_enabled,
                override_active: st.override_until.is_some(),
                override_remaining_secs: remaining,
                effective_target: st.override_percent.unwrap_or(st.cfg.target),
                config_target: st.cfg.target,
                hysteresis: st.cfg.hysteresis,
                max_temp_c: st.cfg.max_temp_c,
                last_action: st.last_action.clone(),
            };
            serde_json::to_value(view)
                .unwrap_or_else(|e| serde_json::json!({"ok": false, "error": format!("{e}")}))
        }
        Request::Override { percent, duration_secs } => {
            st.override_percent = Some(percent);
            st.override_until = Some(Instant::now() + Duration::from_secs(duration_secs));
            st.last_action = format!("override {}% for {}s", percent, duration_secs);
            let _ = tick(&mut st);
            serde_json::json!({"ok": true, "override_percent": percent, "duration_secs": duration_secs})
        }
        Request::CancelOverride => {
            st.override_percent = None;
            st.override_until = None;
            st.last_action = "override cancelled".into();
            let _ = tick(&mut st);
            serde_json::json!({"ok": true})
        }
        Request::SetTarget { target, hysteresis } => {
            if target > 100 {
                return serde_json::json!({"ok": false, "error": "target > 100"});
            }
            st.cfg.target = target;
            if let Some(h) = hysteresis {
                st.cfg.hysteresis = h;
            }
            let _ = save_config(&st.cfg);
            st.last_action = format!("target set to {}%", target);
            let _ = tick(&mut st);
            serde_json::json!({"ok": true, "target": target, "hysteresis": st.cfg.hysteresis})
        }
        Request::Reload => {
            st.cfg = load_config();
            st.last_action = "reloaded config".into();
            let _ = tick(&mut st);
            serde_json::json!({"ok": true, "target": st.cfg.target})
        }
    }
}
