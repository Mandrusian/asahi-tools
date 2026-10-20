use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use asahi_core::{
    find_fan_hwmon, get_battery_capacity, get_battery_status, get_charge_thresholds,
    get_kbd_brightness, get_kbd_max_brightness, scan_fans, scan_temps,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};

const TOOLS: &[(&str, &str, bool)] = &[
    ("kbd-backlight", "Keyboard backlight brightness", true),
    ("battery-limiter", "Battery charge thresholds", true),
    ("net-restarter", "Wi-Fi & Bluetooth info / restart", false),
    ("sys-monitor", "Temperatures & fan control (daemon)", true),
];

const REFRESH: Duration = Duration::from_secs(3);
const LAUNCH_DEBOUNCE: Duration = Duration::from_millis(500);
const QUICK_EXIT_THRESHOLD: Duration = Duration::from_secs(1);
const QUICK_EXIT_HOLD: Duration = Duration::from_secs(2);

static IS_ROOT: OnceLock<bool> = OnceLock::new();
static SUDO_OK: OnceLock<bool> = OnceLock::new();

fn is_root() -> bool {
    *IS_ROOT.get_or_init(|| {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines().find_map(|l| {
                    l.strip_prefix("Uid:")
                        .and_then(|r| r.split_whitespace().next())
                        .and_then(|v| v.parse::<u32>().ok())
                })
            })
            .map(|uid| uid == 0)
            .unwrap_or(false)
    })
}

fn sudo_ok() -> bool {
    *SUDO_OK.get_or_init(can_sudo_nopasswd)
}

// Returns true if the user can run at least one of our privileged binaries
// via passwordless sudo. Checks specific binaries because a general
// `sudo -n true` fails even when /etc/sudoers.d has NOPASSWD rules for
// individual paths (the correct way to do it).
fn can_sudo_nopasswd() -> bool {
    // General NOPASSWD (rare, but honor it if present).
    let general = Command::new("sudo")
        .args(["-n", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if general {
        return true;
    }
    // Specific-binary NOPASSWD: `sudo -n -l <path>` returns 0 iff allowed.
    let dir = current_dir();
    [
        "kbd-backlight",
        "battery-limiter",
        "sys-monitor",
        "sysmon-daemon",
    ]
    .iter()
    .any(|name| {
        let bin = dir.join(name);
        if !bin.exists() {
            return false;
        }
        Command::new("sudo")
            .arg("-n")
            .arg("-l")
            .arg(&bin)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

// ---------------- snapshot ----------------

#[derive(Default, Clone)]
struct Snapshot {
    hostname: String,
    uptime: String,
    load: String,
    mem_used_gb: f64,
    mem_total_gb: f64,
    battery_pct: Option<u32>,
    battery_status: String,
    battery_health: String,
    battery_cycles: Option<u32>,
    thresholds: Option<(u32, u32)>,
    kbd: Option<(u32, u32)>,
    max_temp: f64,
    fan_rpm: Option<u32>,
    fan_target: Option<u32>,
    wifi_ssid: String,
    wifi_ip: String,
    bt_state: String,
}

fn gather(with_network: bool) -> Snapshot {
    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let uptime = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
        })
        .map(fmt_uptime)
        .unwrap_or_default();
    let load = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(|s| s.to_string()))
        .unwrap_or_default();
    let (mem_used_gb, mem_total_gb) = read_meminfo();

    let battery_pct = get_battery_capacity().ok();
    let battery_status = get_battery_status().unwrap_or_default();
    let battery_health = read_battery_attr("health").unwrap_or_default();
    let battery_cycles = read_battery_attr("cycle_count").and_then(|s| s.parse().ok());
    let thresholds = get_charge_thresholds().ok();

    let kbd = match (get_kbd_brightness(), get_kbd_max_brightness()) {
        (Ok(c), Ok(m)) => Some((c, m)),
        _ => None,
    };

    let (max_temp, fan_rpm, fan_target) = if let Some(hwmon) = find_fan_hwmon() {
        let temps = scan_temps(&hwmon);
        let fans = scan_fans(&hwmon);
        let max_t = temps.iter().map(|t| t.celsius).fold(0.0_f64, f64::max);
        let rpm = fans.first().map(|f| f.rpm);
        let target = fans.first().and_then(|f| f.target);
        (max_t, rpm, target)
    } else {
        (0.0, None, None)
    };

    let (wifi_ssid, wifi_ip, bt_state) = if with_network {
        let (s, ip) = wifi_summary();
        (s, ip, bt_summary())
    } else {
        ("…".into(), "…".into(), "…".into())
    };

    Snapshot {
        hostname,
        uptime,
        load,
        mem_used_gb,
        mem_total_gb,
        battery_pct,
        battery_status,
        battery_health,
        battery_cycles,
        thresholds,
        kbd,
        max_temp,
        fan_rpm,
        fan_target,
        wifi_ssid,
        wifi_ip,
        bt_state,
    }
}

fn read_battery_attr(attr: &str) -> Option<String> {
    std::fs::read_to_string(format!("/sys/class/power_supply/macsmc-battery/{attr}"))
        .ok()
        .map(|s| s.trim().to_string())
}

fn read_meminfo() -> (f64, f64) {
    let text = match std::fs::read_to_string("/proc/meminfo") {
        Ok(t) => t,
        Err(_) => return (0.0, 0.0),
    };
    let mut total = 0u64;
    let mut avail = 0u64;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            avail = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        }
    }
    let to_gb = |kb: u64| (kb as f64) / (1024.0 * 1024.0);
    (to_gb(total.saturating_sub(avail)), to_gb(total))
}

fn fmt_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

fn wifi_summary() -> (String, String) {
    let dev = find_wifi_iface().unwrap_or_default();
    if dev.is_empty() {
        return ("(none)".into(), "-".into());
    }
    let text = Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SSID", "dev", "wifi"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let mut ssid = "-".to_string();
    for line in text.lines() {
        if let Some((act, s)) = line.split_once(':') {
            if act.contains('*') {
                ssid = s.to_string();
                break;
            }
        }
    }
    let ip = Command::new("ip")
        .args(["-j", "addr", "show", &dev])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    #[derive(serde::Deserialize)]
    struct A {
        family: String,
        local: String,
    }
    #[derive(serde::Deserialize)]
    struct L {
        #[serde(default)]
        addr_info: Vec<A>,
    }
    let ip4 = serde_json::from_str::<Vec<L>>(&ip)
        .ok()
        .and_then(|v| {
            v.into_iter().next().and_then(|l| {
                l.addr_info
                    .into_iter()
                    .find(|a| a.family == "inet")
                    .map(|a| a.local)
            })
        })
        .unwrap_or_else(|| "-".into());
    (ssid, ip4)
}

fn bt_summary() -> String {
    let out = Command::new("bluetoothctl")
        .args(["devices", "Connected"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let count = out.lines().filter(|l| l.starts_with("Device ")).count();
    let powered = Command::new("bluetoothctl")
        .arg("show")
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.trim() == "Powered: yes")
        })
        .unwrap_or(false);
    if !powered {
        "off".into()
    } else if count == 0 {
        "on · 0 dev".into()
    } else {
        format!("on · {count} dev")
    }
}

fn find_wifi_iface() -> Option<String> {
    let out = Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE", "dev"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some((dev, ty)) = line.split_once(':') {
            if ty == "wifi" {
                return Some(dev.to_string());
            }
        }
    }
    None
}

// ---------------- async gather thread ----------------

fn spawn_gather_thread() -> Receiver<Snapshot> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || loop {
        let snap = gather(true);
        if tx.send(snap).is_err() {
            break;
        }
        std::thread::sleep(REFRESH);
    });
    rx
}

// ---------------- launching ----------------

fn current_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn drain_events() {
    for _ in 0..256 {
        match event::poll(Duration::ZERO) {
            Ok(true) => {
                let _ = event::read();
            }
            _ => break,
        }
    }
}

fn spawn_daemon(mut cmd: Command) -> bool {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

fn ensure_daemon_running() -> Result<(), String> {
    let sock = Path::new("/run/asahi-sysmon.sock");
    if sock.exists() {
        return Ok(());
    }
    let daemon = current_dir().join("sysmon-daemon");
    if !daemon.exists() {
        return Err(format!("sysmon-daemon not found at {}", daemon.display()));
    }

    let wait_socket = |ms: u64| -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(ms) {
            if sock.exists() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        sock.exists()
    };

    let mut tried: Vec<&'static str> = Vec::new();

    if is_root() {
        tried.push("direct");
        if spawn_daemon(Command::new(&daemon)) && wait_socket(3000) {
            return Ok(());
        }
    }

    if !is_root() && sudo_ok() {
        tried.push("sudo -n");
        let mut c = Command::new("sudo");
        c.arg("-n").arg(&daemon);
        if spawn_daemon(c) && wait_socket(3000) {
            return Ok(());
        }
    }

    tried.push("as user");
    if spawn_daemon(Command::new(&daemon)) && wait_socket(1500) {
        return Ok(());
    }

    Err(format!(
        "daemon failed to start (tried: {}). run `sudo {}` in another terminal.",
        tried.join(", "),
        daemon.display()
    ))
}

fn launch_tool(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    binary: &str,
    needs_root: bool,
) -> io::Result<(String, bool)> {
    let path = current_dir().join(binary);
    if !path.exists() {
        return Ok((
            format!("❌ {binary} not found at {}", path.display()),
            false,
        ));
    }

    if binary == "sys-monitor" {
        if let Err(msg) = ensure_daemon_running() {
            return Ok((format!("❌ sys-monitor: {msg}"), false));
        }
    }

    let use_sudo = needs_root && !is_root() && sudo_ok();
    let sudo_hint = if needs_root && !is_root() && !use_sudo {
        Some("⚠ needs root; password may be required inside the tool")
    } else {
        None
    };

    // 1. Exit TUI and print a launch banner
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    println!();
    println!("  ▶ launching {binary}");
    println!("    {}", path.display());
    if use_sudo {
        println!("    via sudo -n");
    }
    if let Some(h) = sudo_hint {
        println!("    {h}");
    }
    println!();
    io::stdout().flush()?;
    std::thread::sleep(Duration::from_millis(100));

    // 2. Run the tool, timed
    let start = Instant::now();
    let status = if use_sudo {
        Command::new("sudo").arg("-n").arg(&path).status()
    } else {
        Command::new(&path).status()
    };
    let elapsed = start.elapsed();

    let success = status.as_ref().map(|s| s.success()).unwrap_or(false);
    let exit_desc = match &status {
        Ok(s) if s.success() => format!("exit 0, {:.1}s", elapsed.as_secs_f64()),
        Ok(s) => format!("{s}, {:.1}s", elapsed.as_secs_f64()),
        Err(e) => format!("spawn error: {e}"),
    };

    let banner = if success {
        format!("✓ {binary} closed ({exit_desc})")
    } else {
        format!("⚠ {binary} ({exit_desc})")
    };

    // 3. Hold the screen for quick exits / failures so it isn't a flash
    if !success || elapsed < QUICK_EXIT_THRESHOLD {
        println!("  {banner}");
        println!();
        println!(
            "  returning to asahi-tui in {}s ...",
            QUICK_EXIT_HOLD.as_secs()
        );
        io::stdout().flush()?;
        std::thread::sleep(QUICK_EXIT_HOLD);
    }

    // 4. Restore TUI
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;
    terminal.hide_cursor()?;
    drain_events();

    Ok((banner, success))
}

// ---------------- UI ----------------

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.hide_cursor()?;

    let res = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    // Warm caches before first paint so the first frame is instant.
    let _ = is_root();
    let _ = sudo_ok();

    let mut selected = 0usize;
    let mut status = String::new();
    let mut status_color = Color::DarkGray;
    // Fast synchronous snapshot (no subprocesses) — network fields show "…"
    let mut snap = gather(false);
    let mut last_launch = Instant::now() - Duration::from_secs(60);
    let rx = spawn_gather_thread();

    loop {
        // Drain any async snapshots that have arrived.
        while let Ok(s) = rx.try_recv() {
            snap = s;
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ])
                .split(f.area());

            let header = Paragraph::new(Line::from(vec![
                Span::styled(
                    "asahi-tools",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("   "),
                Span::styled(&snap.hostname, Style::default().fg(Color::Gray)),
                Span::raw("   "),
                Span::styled(
                    format!("uptime {}", snap.uptime),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("   "),
                if is_root() {
                    Span::styled("running as root", Style::default().fg(Color::Yellow))
                } else if sudo_ok() {
                    Span::styled("sudo NOPASSWD ok", Style::default().fg(Color::Green))
                } else {
                    Span::styled(
                        "no passwordless sudo — writes may fail",
                        Style::default().fg(Color::Red),
                    )
                },
            ]))
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(chunks[1]);

            let items: Vec<ListItem> = TOOLS
                .iter()
                .enumerate()
                .map(|(i, (name, desc, needs_root))| {
                    let sel = i == selected;
                    let prefix = if sel { "▶ " } else { "  " };
                    let style = if sel {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let marker = if *needs_root { " [root]" } else { "" };
                    ListItem::new(vec![
                        Line::from(Span::styled(format!("{prefix}{name}{marker}"), style)),
                        Line::from(Span::styled(
                            format!("    {desc}"),
                            Style::default().fg(Color::DarkGray),
                        )),
                    ])
                })
                .collect();
            let mut state = ListState::default();
            state.select(Some(selected));
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" tools "))
                .highlight_style(Style::default().bg(Color::DarkGray));
            f.render_stateful_widget(list, cols[0], &mut state);

            let mut info: Vec<Line> = Vec::new();
            info.push(section("system"));
            info.push(kv("load", &snap.load));
            info.push(kv(
                "memory",
                &format!("{:.1} / {:.1} GiB", snap.mem_used_gb, snap.mem_total_gb),
            ));
            info.push(Line::from(""));
            info.push(section("battery"));
            if let Some(pct) = snap.battery_pct {
                info.push(kv("charge", &format!("{pct}% ({})", snap.battery_status)));
            } else {
                info.push(kv("charge", "unknown"));
            }
            if !snap.battery_health.is_empty() {
                let mut s = snap.battery_health.clone();
                if let Some(c) = snap.battery_cycles {
                    s.push_str(&format!("  · {c} cycles"));
                }
                info.push(kv("health", &s));
            }
            if let Some((s, e)) = snap.thresholds {
                info.push(kv("thresholds", &format!("{s}-{e}%")));
            }
            info.push(Line::from(""));
            info.push(section("thermal"));
            info.push(kv("max temp", &format!("{:.1} °C", snap.max_temp)));
            match (snap.fan_rpm, snap.fan_target) {
                (Some(rpm), Some(t)) => info.push(kv("fan 1", &format!("{rpm} rpm (target {t})"))),
                (Some(rpm), None) => info.push(kv("fan 1", &format!("{rpm} rpm (auto)"))),
                _ => info.push(kv("fan 1", "n/a (is hwmon loaded?)")),
            }
            info.push(Line::from(""));
            info.push(section("network"));
            info.push(kv("wifi", &format!("{}  {}", snap.wifi_ssid, snap.wifi_ip)));
            info.push(kv("bluetooth", &snap.bt_state));
            info.push(Line::from(""));
            info.push(section("input"));
            if let Some((c, m)) = snap.kbd {
                info.push(kv("kbd backlight", &format!("{c} / {m}")));
            } else {
                info.push(kv("kbd backlight", "not detected"));
            }

            let info_w = Paragraph::new(info).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" at a glance "),
            );
            f.render_widget(info_w, cols[1]);

            let help = Paragraph::new(Line::from(vec![
                Span::styled("↑/↓", Style::default().fg(Color::Green)),
                Span::raw(" select · "),
                Span::styled("enter", Style::default().fg(Color::Green)),
                Span::raw(" launch · "),
                Span::styled("r", Style::default().fg(Color::Green)),
                Span::raw(" refresh · "),
                Span::styled("q", Style::default().fg(Color::Green)),
                Span::raw(" quit  "),
                Span::styled(&status, Style::default().fg(status_color)),
            ]))
            .block(Block::default().borders(Borders::ALL).title(" keys "));
            f.render_widget(help, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Up => {
                        if selected > 0 {
                            selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if selected + 1 < TOOLS.len() {
                            selected += 1;
                        }
                    }
                    KeyCode::Char('r') => {
                        snap = gather(false);
                        status = "refreshed".into();
                        status_color = Color::DarkGray;
                    }
                    KeyCode::Enter => {
                        if last_launch.elapsed() < LAUNCH_DEBOUNCE {
                            status = "ignored (debounce)".into();
                            status_color = Color::Yellow;
                            continue;
                        }
                        last_launch = Instant::now();
                        let (name, _, needs_root) = TOOLS[selected];
                        let (msg, ok) = launch_tool(terminal, name, needs_root)?;
                        status = msg;
                        status_color = if ok { Color::Green } else { Color::Red };
                        snap = gather(false);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn section(name: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("── {name} ──"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn kv(k: &str, v: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {k:<12}"), Style::default().fg(Color::DarkGray)),
        Span::raw(v.to_string()),
    ])
}
