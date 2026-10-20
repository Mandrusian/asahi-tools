use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use asahi_core::{
    charge, find_fan_hwmon, get_battery_capacity, get_battery_status, get_kbd_brightness,
    get_kbd_max_brightness, history, scan_fans, scan_temps, Fan, TempSensor,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Sparkline, Tabs},
    Terminal,
};

const TOOLS: &[(&str, &str, &str, bool)] = &[
    ("Power",   "asahi-battery-health",  "Battery history sparkline",      false),
    ("Power",   "asahi-power-draw",      "Live power rails (SMC)",         true),
    ("Thermal", "sys-monitor",           "Thermal TUI + fan control",      true),
    ("Input",   "asahi-trackpad-viz",    "Touch + pressure visualizer",    true),
    ("Input",   "asahi-kbd-fn-remap",    "F1-F12 vs media keys",           false),
    ("Network", "net-restarter",         "Wi-Fi/BT scan + restart",        false),
    ("Network", "asahi-wifi-log",        "Log WiFi drops",                 false),
    ("Display", "asahi-display",         "Switch display outputs",         false),
    ("System",  "asahi-kernel-manager",  "Safe kernel updates",            false),
    ("System",  "asahi-snapshot",        "Snapshot / rollback",            false),
    ("System",  "asahi-sensor-explorer", "List all hwmon inputs",          false),
    ("System",  "asahi-diag",            "Diagnostic bundle",              false),
    ("GUI",     "asahi-gtk",             "Asahi Settings (GTK4)",          false),
];

const REFRESH: Duration = Duration::from_secs(3);
const LAUNCH_DEBOUNCE: Duration = Duration::from_millis(500);

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
    *SUDO_OK.get_or_init(|| {
        Command::new("sudo")
            .args(["-n", "true"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
            || ["kbd-backlight", "battery-limiter", "sys-monitor", "sysmon-daemon"]
                .iter()
                .any(|name| {
                    let bin = current_dir().join(name);
                    bin.exists()
                        && Command::new("sudo")
                            .args(["-n", "-l"])
                            .arg(&bin)
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false)
                })
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Tools,
    Battery,
    Thermal,
    Network,
}

impl Tab {
    fn index(self) -> usize {
        match self {
            Tab::Tools => 0,
            Tab::Battery => 1,
            Tab::Thermal => 2,
            Tab::Network => 3,
        }
    }
}

const TAB_LABELS: &[&str] = &[" Tools ", " Battery ", " Thermal ", " Network "];

#[derive(Default, Clone)]
struct Snapshot {
    hostname: String,
    uptime: String,
    load: String,
    mem_used_gb: f64,
    mem_total_gb: f64,
    battery_pct: Option<u32>,
    battery_state: String,
    temps: Vec<TempSensor>,
    fans: Vec<Fan>,
    hwmon_ok: bool,
    wifi_ssid: String,
    wifi_ip: String,
    wifi_mac: String,
    wifi_signal: u32,
    bt_state: String,
    bt_connected: usize,
    kbd: Option<(u32, u32)>,
}

fn gather() -> Snapshot {
    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let uptime = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|x| x.parse::<u64>().ok()))
        .map(fmt_uptime)
        .unwrap_or_default();
    let load = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(|s| s.to_string()))
        .unwrap_or_default();
    let (mem_used_gb, mem_total_gb) = read_meminfo();
    let battery_pct = get_battery_capacity().ok();
    let battery_state = get_battery_status().unwrap_or_default();
    let (temps, fans, hwmon_ok) = match find_fan_hwmon() {
        Some(h) => (scan_temps(&h), scan_fans(&h), true),
        None => (Vec::new(), Vec::new(), false),
    };
    let (wifi_ssid, wifi_ip, wifi_mac, wifi_signal) = wifi_info();
    let (bt_state, bt_connected) = bt_info();
    let kbd = match (get_kbd_brightness(), get_kbd_max_brightness()) {
        (Ok(c), Ok(m)) => Some((c, m)),
        _ => None,
    };
    Snapshot {
        hostname, uptime, load, mem_used_gb, mem_total_gb, battery_pct, battery_state,
        temps, fans, hwmon_ok, wifi_ssid, wifi_ip, wifi_mac, wifi_signal, bt_state,
        bt_connected, kbd,
    }
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

fn read_meminfo() -> (f64, f64) {
    let text = match std::fs::read_to_string("/proc/meminfo") {
        Ok(t) => t,
        Err(_) => return (0.0, 0.0),
    };
    let mut total = 0u64;
    let mut avail = 0u64;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = rest.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            avail = rest.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
        }
    }
    let to_gb = |kb: u64| (kb as f64) / (1024.0 * 1024.0);
    (to_gb(total.saturating_sub(avail)), to_gb(total))
}

fn find_wifi_iface() -> Option<String> {
    let out = Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE", "dev"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| {
            let (dev, ty) = l.split_once(':')?;
            (ty == "wifi").then(|| dev.to_string())
        })
}

fn wifi_info() -> (String, String, String, u32) {
    let dev = match find_wifi_iface() {
        Some(d) => d,
        None => return ("(none)".into(), "-".into(), "-".into(), 0),
    };
    let (ssid, signal) = match Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SSID,SIGNAL", "dev", "wifi"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .find_map(|l| {
                let mut it = l.rsplitn(3, ':');
                let sig = it.next()?.parse::<u32>().ok()?;
                let ssid = it.next()?.to_string();
                let act = it.next()?;
                act.contains('*').then_some((ssid, sig))
            })
            .unwrap_or(("-".into(), 0)),
        _ => ("-".into(), 0),
    };
    #[derive(serde::Deserialize, Default)]
    struct Addr {
        family: String,
        local: String,
    }
    #[derive(serde::Deserialize, Default)]
    struct Link {
        address: Option<String>,
        #[serde(default)]
        addr_info: Vec<Addr>,
    }
    let (ip, mac) = match Command::new("ip").args(["-j", "addr", "show", &dev]).output() {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let link = serde_json::from_str::<Vec<Link>>(&text)
                .unwrap_or_default()
                .into_iter()
                .next()
                .unwrap_or_default();
            let ip = link
                .addr_info
                .iter()
                .find(|a| a.family == "inet")
                .map(|a| a.local.clone())
                .unwrap_or_else(|| "-".into());
            (ip, link.address.unwrap_or_else(|| "-".into()))
        }
        _ => ("-".into(), "-".into()),
    };
    (ssid, ip, mac, signal)
}

fn bt_info() -> (String, usize) {
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
        return ("off".into(), 0);
    }
    let count = Command::new("bluetoothctl")
        .args(["devices", "Connected"])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| l.starts_with("Device "))
                .count()
        })
        .unwrap_or(0);
    ("on".into(), count)
}

fn cap_color(pct: u32) -> Color {
    if pct < 20 { Color::Red } else if pct < 40 { Color::Yellow } else { Color::Green }
}

fn temp_color(t: f64) -> Color {
    if t > 80.0 { Color::Red } else if t > 60.0 { Color::Yellow } else { Color::Green }
}

fn signal_color(s: u32) -> Color {
    if s >= 70 { Color::Green } else if s >= 40 { Color::Yellow } else { Color::Red }
}

fn draw_tools(
    f: &mut ratatui::Frame,
    area: Rect,
    selected: usize,
    snap: &Snapshot,
    cs: Option<&charge::Status>,
) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(area);

    let items: Vec<ListItem> = TOOLS
        .iter()
        .enumerate()
        .map(|(i, (cat, name, desc, needs_root))| {
            let sel = i == selected;
            let cursor = if sel { "▶" } else { " " };
            let name_style = if sel {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let suffix = if *needs_root { " [root]" } else { "" };
            ListItem::new(Line::from(vec![
                Span::styled(cursor, Style::default().fg(Color::Cyan)),
                Span::styled(format!(" {:<8}", cat), Style::default().fg(Color::DarkGray)),
                Span::styled(format!(" {:<22}", name), name_style),
                Span::styled(format!("{}{}", desc, suffix), Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(selected));
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Tools ({}) — ↑/↓ select, enter launch ",
            TOOLS.len()
        )))
        .highlight_style(Style::default().bg(Color::DarkGray));
    f.render_stateful_widget(list, cols[0], &mut state);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        "System",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(format!("  {}", snap.hostname)));
    lines.push(Line::from(format!("  up {} · load {}", snap.uptime, snap.load)));
    lines.push(Line::from(format!(
        "  mem {:.1} / {:.1} GiB",
        snap.mem_used_gb, snap.mem_total_gb
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Now",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )));
    if let Some(pct) = snap.battery_pct {
        let state = match cs {
            Some(c) if c.override_active => "override",
            Some(c) if c.charging_enabled => "charging",
            Some(_) => "paused",
            None => "offline",
        };
        lines.push(Line::from(vec![
            Span::raw("  battery "),
            Span::styled(
                format!("{}%", pct),
                Style::default().fg(cap_color(pct)).add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" · {}", state)),
        ]));
    }
    if snap.hwmon_ok {
        let max_t = snap.temps.iter().map(|t| t.celsius).fold(0.0_f64, f64::max);
        lines.push(Line::from(vec![
            Span::raw("  thermal "),
            Span::styled(
                format!("{:.1} °C", max_t),
                Style::default().fg(temp_color(max_t)),
            ),
        ]));
    }
    lines.push(Line::from(format!("  wifi    {}", snap.wifi_ssid)));
    if let Some((c, m)) = snap.kbd {
        lines.push(Line::from(format!("  kbd     {} / {}", c, m)));
    }
    let w = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Status "));
    f.render_widget(w, cols[1]);
}

fn draw_battery(f: &mut ratatui::Frame, area: Rect, snap: &Snapshot, cs: Option<&charge::Status>, hist: &[u32]) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Length(5), Constraint::Min(5)])
        .split(area);

    let mut h: Vec<Line> = Vec::new();
    match cs {
        Some(c) => {
            let pct = snap.battery_pct.unwrap_or(0);
            let (state_str, sc) = if c.override_active {
                (format!("OVERRIDE · {} left", charge::fmt_duration(c.override_remaining_secs.unwrap_or(0))), Color::Yellow)
            } else if c.charging_enabled {
                ("charging".to_string(), Color::Green)
            } else {
                ("paused".to_string(), Color::Cyan)
            };
            let t = c.temp_c.map(|t| format!("{:.1} °C", t)).unwrap_or_else(|| "-".into());
            let st = if snap.battery_state.is_empty() { "-" } else { &snap.battery_state };
            h.push(Line::from(vec![
                Span::raw("capacity "),
                Span::styled(format!("{}%", pct), Style::default().fg(cap_color(pct)).add_modifier(Modifier::BOLD)),
                Span::raw("   state "),
                Span::styled(state_str, Style::default().fg(sc).add_modifier(Modifier::BOLD)),
                Span::raw(format!("   temp {}   status {}", t, st)),
            ]));
            h.push(Line::from(vec![
                Span::raw("target "),
                Span::styled(format!("{}%", c.effective_target), Style::default().fg(Color::Cyan)),
                Span::raw(format!("   config {}%   hysteresis {}%   max {}°C", c.config_target, c.hysteresis, c.max_temp_c)),
            ]));
            h.push(Line::from(vec![
                Span::raw("last action "),
                Span::styled(c.last_action.as_str(), Style::default().fg(Color::DarkGray)),
            ]));
            h.push(Line::from(""));
            h.push(Line::from(Span::styled(
                "o override 100% 2h   O override 100% 24h   c cancel   t cycle target",
                Style::default().fg(Color::Gray),
            )));
        }
        None => {
            h.push(Line::from(Span::styled(
                "charge daemon not reachable",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
            h.push(Line::from(""));
            h.push(Line::from("  sudo systemctl status asahi-charge-daemon"));
            h.push(Line::from("  (unit execs /usr/local/bin/asahi-charge-daemon)"));
        }
    }
    f.render_widget(Paragraph::new(h).block(Block::default().borders(Borders::ALL).title(" Charge control ")), rows[0]);

    let cfg = vec![
        Line::from(Span::styled("Defaults", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(format!(
            "  target {}%   hysteresis {}%   max temp {}°C",
            cs.map(|c| c.config_target).unwrap_or(80),
            cs.map(|c| c.hysteresis).unwrap_or(2),
            cs.map(|c| c.max_temp_c).unwrap_or(45),
        )),
        Line::from("  file: /etc/asahi-tools/charge-daemon.conf"),
    ];
    f.render_widget(Paragraph::new(cfg).block(Block::default().borders(Borders::ALL)), rows[1]);

    let data: Vec<u64> = hist.iter().map(|c| *c as u64).collect();
    let spark = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Capacity history ({} samples · asahi-battery-health) ", hist.len()
        )))
        .data(&data[..])
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(spark, rows[2]);
}

fn draw_thermal(f: &mut ratatui::Frame, area: Rect, snap: &Snapshot) {
    if !snap.hwmon_ok {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled("macsmc-hwmon not loaded", Style::default().fg(Color::Red))),
                Line::from(""),
                Line::from("Try: sudo modprobe macsmc_hwmon"),
            ])
            .block(Block::default().borders(Borders::ALL).title(" Thermal ")),
            area,
        );
        return;
    }
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let mut t: Vec<Line> = Vec::new();
    t.push(Line::from(Span::styled(
        format!("{:<16} {:>8}", "sensor", "temp"),
        Style::default().fg(Color::DarkGray),
    )));
    for s in &snap.temps {
        t.push(Line::from(vec![
            Span::raw(format!("{:<16} ", s.name)),
            Span::styled(format!("{:>6.1} °C", s.celsius), Style::default().fg(temp_color(s.celsius))),
        ]));
    }
    f.render_widget(Paragraph::new(t).block(Block::default().borders(Borders::ALL).title(" Temperatures ")), cols[0]);

    let mut fan_lines: Vec<Line> = Vec::new();
    if snap.fans.is_empty() {
        fan_lines.push(Line::from("no fans detected"));
    } else {
        fan_lines.push(Line::from(Span::styled(
            format!("{:<8} {:>8} {:>10}", "fan", "current", "target"),
            Style::default().fg(Color::DarkGray),
        )));
        for fan in &snap.fans {
            let target = fan.target.map(|t| t.to_string()).unwrap_or_else(|| "auto".into());
            fan_lines.push(Line::from(format!(
                "{:<8} {:>6} rpm {:>10}",
                format!("fan {}", fan.index),
                fan.rpm,
                target
            )));
        }
    }
    fan_lines.push(Line::from(""));
    fan_lines.push(Line::from(Span::styled("Press s to launch sys-monitor", Style::default().fg(Color::DarkGray))));
    f.render_widget(Paragraph::new(fan_lines).block(Block::default().borders(Borders::ALL).title(" Fans ")), cols[1]);
}

fn draw_network(f: &mut ratatui::Frame, area: Rect, snap: &Snapshot) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Length(5), Constraint::Min(2)])
        .split(area);

    let wifi = vec![
        Line::from(vec![Span::styled("SSID      ", Style::default().fg(Color::DarkGray)), Span::raw(&snap.wifi_ssid)]),
        Line::from(vec![Span::styled("IPv4      ", Style::default().fg(Color::DarkGray)), Span::raw(&snap.wifi_ip)]),
        Line::from(vec![Span::styled("MAC       ", Style::default().fg(Color::DarkGray)), Span::raw(&snap.wifi_mac)]),
        Line::from(vec![
            Span::styled("signal    ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}%", snap.wifi_signal), Style::default().fg(signal_color(snap.wifi_signal))),
        ]),
    ];
    f.render_widget(Paragraph::new(wifi).block(Block::default().borders(Borders::ALL).title(" Wi-Fi ")), rows[0]);

    let bt = vec![
        Line::from(vec![Span::styled("adapter    ", Style::default().fg(Color::DarkGray)), Span::raw(&snap.bt_state)]),
        Line::from(vec![
            Span::styled("connected  ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{} device(s)", snap.bt_connected)),
        ]),
    ];
    f.render_widget(Paragraph::new(bt).block(Block::default().borders(Borders::ALL).title(" Bluetooth ")), rows[1]);

    f.render_widget(
        Paragraph::new(Span::styled("Press n to launch net-restarter for scan + device list", Style::default().fg(Color::DarkGray)))
            .block(Block::default().borders(Borders::ALL).title(" more ")),
        rows[2],
    );
}

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

fn ensure_sysmon_daemon() -> Result<(), String> {
    let sock = Path::new("/run/asahi-sysmon.sock");
    if sock.exists() {
        return Ok(());
    }
    let daemon = current_dir().join("sysmon-daemon");
    if !daemon.exists() {
        return Err(format!("sysmon-daemon not found at {}", daemon.display()));
    }
    let spawn = |mut c: Command| -> bool {
        c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
    };
    let wait = |ms: u64| -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(ms) {
            if sock.exists() { return true; }
            std::thread::sleep(Duration::from_millis(100));
        }
        sock.exists()
    };
    if is_root() && spawn(Command::new(&daemon)) && wait(3000) {
        return Ok(());
    }
    if !is_root() && sudo_ok() {
        let mut c = Command::new("sudo");
        c.arg("-n").arg(&daemon);
        if spawn(c) && wait(3000) {
            return Ok(());
        }
    }
    if spawn(Command::new(&daemon)) && wait(1500) {
        return Ok(());
    }
    Err("daemon failed to start".into())
}

fn launch_tool(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    binary: &str,
    needs_root: bool,
) -> io::Result<(String, bool)> {
    let path = current_dir().join(binary);
    if !path.exists() {
        return Ok((format!("✗ {binary} not found"), false));
    }
    if binary == "sys-monitor" {
        if let Err(msg) = ensure_sysmon_daemon() {
            return Ok((format!("✗ sys-monitor: {msg}"), false));
        }
    }
    let use_sudo = needs_root && !is_root() && sudo_ok();
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    println!();
    println!("  ▶ launching {binary}");
    println!("    {}", path.display());
    if use_sudo { println!("    via sudo -n"); }
    println!();
    io::stdout().flush()?;
    std::thread::sleep(Duration::from_millis(100));
    let start = Instant::now();
    let status = if use_sudo {
        Command::new("sudo").arg("-n").arg(&path).status()
    } else {
        Command::new(&path).status()
    };
    let elapsed = start.elapsed();
    let success = status.as_ref().map(|s| s.success()).unwrap_or(false);
    let banner = if success {
        format!("✓ {binary} closed ({:.1}s)", elapsed.as_secs_f64())
    } else {
        format!("⚠ {binary} ({:.1}s)", elapsed.as_secs_f64())
    };
    if !success || elapsed < Duration::from_secs(1) {
        println!("  {banner}");
        println!();
        println!("  returning to asahi-tui in 2s ...");
        io::stdout().flush()?;
        std::thread::sleep(Duration::from_secs(2));
    }
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;
    terminal.hide_cursor()?;
    drain_events();
    Ok((banner, success))
}

/// Ensure sudo credentials are cached for this session.
/// Prints one prompt if needed, before entering alternate screen.
/// If the user Ctrl+C's, we continue — write operations will show red
/// warnings but read-only features still work.
fn ensure_sudo_cached() {
    if is_root() {
        return;
    }
    // Already cached (or NOPASSWD)? done.
    let cached = Command::new("sudo")
        .args(["-n", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if cached {
        return;
    }
    // Prompt once
    eprintln!();
    eprintln!("  asahi-tui needs sudo for:");
    eprintln!("    - starting/stopping fan and charge daemons");
    eprintln!("    - running trackpad calibration");
    eprintln!("    - launching system tools that write to sysfs");
    eprintln!();
    eprintln!("  Enter your password once — cached for ~15 min.");
    eprintln!("  Press Ctrl+C to skip (some features will show warnings).");
    eprintln!();
    let _ = Command::new("sudo").arg("-v").status();
}

fn main() -> io::Result<()> {
    ensure_sudo_cached();
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
    let _ = is_root();
    let _ = sudo_ok();
    let mut tab = Tab::Tools;
    let mut selected: usize = 0;
    let mut status = String::new();
    let mut status_color = Color::DarkGray;
    let mut snap = gather();
    let mut cs = charge::status();
    let mut hist = history::load(120);
    let mut last_refresh = Instant::now();
    let mut last_charge = Instant::now();
    let mut last_launch = Instant::now() - Duration::from_secs(60);

    loop {
        if last_refresh.elapsed() > REFRESH {
            snap = gather();
            hist = history::load(120);
            last_refresh = Instant::now();
        }
        if last_charge.elapsed() > Duration::from_secs(2) {
            cs = charge::status();
            last_charge = Instant::now();
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(10), Constraint::Length(3)])
                .split(f.area());
            let tabs = Tabs::new(TAB_LABELS.iter().copied())
                .select(tab.index())
                .style(Style::default().fg(Color::Gray))
                .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .block(Block::default().borders(Borders::ALL).title(format!(" asahi-tools v{} ", env!("CARGO_PKG_VERSION"))));
            f.render_widget(tabs, chunks[0]);
            match tab {
                Tab::Tools => draw_tools(f, chunks[1], selected, &snap, cs.as_ref()),
                Tab::Battery => draw_battery(f, chunks[1], &snap, cs.as_ref(), &hist),
                Tab::Thermal => draw_thermal(f, chunks[1], &snap),
                Tab::Network => draw_network(f, chunks[1], &snap),
            }
            let help: Vec<Span> = match tab {
                Tab::Tools => vec![
                    Span::styled("↑/↓ ", Style::default().fg(Color::Green)),
                    Span::raw("select · "),
                    Span::styled("enter ", Style::default().fg(Color::Green)),
                    Span::raw("launch · "),
                    Span::styled("tab ", Style::default().fg(Color::Green)),
                    Span::raw("next · "),
                    Span::styled("q ", Style::default().fg(Color::Green)),
                    Span::raw("quit  "),
                    Span::styled(&status, Style::default().fg(status_color)),
                ],
                Tab::Battery => vec![
                    Span::styled("o ", Style::default().fg(Color::Green)),
                    Span::raw("2h override · "),
                    Span::styled("O ", Style::default().fg(Color::Green)),
                    Span::raw("24h · "),
                    Span::styled("c ", Style::default().fg(Color::Green)),
                    Span::raw("cancel · "),
                    Span::styled("t ", Style::default().fg(Color::Green)),
                    Span::raw("cycle target · "),
                    Span::styled("q ", Style::default().fg(Color::Green)),
                    Span::raw("quit  "),
                    Span::styled(&status, Style::default().fg(status_color)),
                ],
                Tab::Thermal => vec![
                    Span::styled("s ", Style::default().fg(Color::Green)),
                    Span::raw("sys-monitor · "),
                    Span::styled("r ", Style::default().fg(Color::Green)),
                    Span::raw("refresh · "),
                    Span::styled("q ", Style::default().fg(Color::Green)),
                    Span::raw("quit  "),
                    Span::styled(&status, Style::default().fg(status_color)),
                ],
                Tab::Network => vec![
                    Span::styled("n ", Style::default().fg(Color::Green)),
                    Span::raw("net-restarter · "),
                    Span::styled("r ", Style::default().fg(Color::Green)),
                    Span::raw("refresh · "),
                    Span::styled("q ", Style::default().fg(Color::Green)),
                    Span::raw("quit  "),
                    Span::styled(&status, Style::default().fg(status_color)),
                ],
            };
            f.render_widget(
                Paragraph::new(Line::from(help)).block(Block::default().borders(Borders::ALL).title(" keys ")),
                chunks[2],
            );
        })?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press { continue; }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab => {
                        tab = match tab {
                            Tab::Tools => Tab::Battery,
                            Tab::Battery => Tab::Thermal,
                            Tab::Thermal => Tab::Network,
                            Tab::Network => Tab::Tools,
                        };
                    }
                    KeyCode::Char('1') => tab = Tab::Tools,
                    KeyCode::Char('2') => tab = Tab::Battery,
                    KeyCode::Char('3') => tab = Tab::Thermal,
                    KeyCode::Char('4') => tab = Tab::Network,
                    KeyCode::Up => { if tab == Tab::Tools && selected > 0 { selected -= 1; } }
                    KeyCode::Down => { if tab == Tab::Tools && selected + 1 < TOOLS.len() { selected += 1; } }
                    KeyCode::Char('r') => {
                        snap = gather();
                        cs = charge::status();
                        hist = history::load(120);
                        last_refresh = Instant::now();
                        last_charge = Instant::now();
                        status = "refreshed".into();
                        status_color = Color::DarkGray;
                    }
                    KeyCode::Char('o') if tab == Tab::Battery => {
                        match charge::override_for(100, 7200) {
                            Ok(()) => { status = "override 100% 2h".into(); status_color = Color::Green; }
                            Err(e) => { status = format!("override failed: {e}"); status_color = Color::Red; }
                        }
                        cs = charge::status();
                    }
                    KeyCode::Char('O') if tab == Tab::Battery => {
                        match charge::override_for(100, 86400) {
                            Ok(()) => { status = "override 100% 24h".into(); status_color = Color::Green; }
                            Err(e) => { status = format!("override failed: {e}"); status_color = Color::Red; }
                        }
                        cs = charge::status();
                    }
                    KeyCode::Char('c') if tab == Tab::Battery => {
                        match charge::cancel_override() {
                            Ok(()) => { status = "override cancelled".into(); status_color = Color::Green; }
                            Err(e) => { status = format!("cancel failed: {e}"); status_color = Color::Red; }
                        }
                        cs = charge::status();
                    }
                    KeyCode::Char('t') if tab == Tab::Battery => {
                        let current = cs.as_ref().map(|c| c.config_target).unwrap_or(80);
                        let next = match current { 60 => 70, 70 => 80, 80 => 85, 85 => 90, 90 => 100, _ => 60 };
                        match charge::set_target(next, None) {
                            Ok(()) => { status = format!("target → {next}%"); status_color = Color::Green; }
                            Err(e) => { status = format!("set-target failed: {e}"); status_color = Color::Red; }
                        }
                        cs = charge::status();
                    }
                    KeyCode::Char('s') if tab == Tab::Thermal => {
                        let (msg, ok) = launch_tool(terminal, "sys-monitor", true)?;
                        status = msg;
                        status_color = if ok { Color::Green } else { Color::Red };
                        snap = gather();
                    }
                    KeyCode::Char('n') if tab == Tab::Network => {
                        let (msg, ok) = launch_tool(terminal, "net-restarter", false)?;
                        status = msg;
                        status_color = if ok { Color::Green } else { Color::Red };
                        snap = gather();
                    }
                    KeyCode::Enter if tab == Tab::Tools => {
                        if last_launch.elapsed() < LAUNCH_DEBOUNCE {
                            status = "ignored (debounce)".into();
                            status_color = Color::Yellow;
                            continue;
                        }
                        last_launch = Instant::now();
                        let (_, name, _, needs_root) = TOOLS[selected];
                        let (msg, ok) = launch_tool(terminal, name, needs_root)?;
                        status = msg;
                        status_color = if ok { Color::Green } else { Color::Red };
                        snap = gather();
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
