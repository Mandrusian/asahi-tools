use std::collections::HashMap;
use std::io;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
    Terminal,
};
use serde::Deserialize;

// ---------------- data ----------------

#[derive(Deserialize, Debug, Clone, Default)]
struct IpAddr {
    family: String,
    local: String,
    prefixlen: u32,
}

#[derive(Deserialize, Debug, Clone, Default)]
struct IpLink {
    ifname: String,
    address: Option<String>,
    operstate: Option<String>,
    #[serde(default)]
    addr_info: Vec<IpAddr>,
}

#[derive(Debug, Clone, Default)]
struct WifiNet {
    ssid: String,
    signal: u32,
    security: String,
    freq_mhz: u32,
    bssid: String,
    rate: String,
    active: bool,
}

#[derive(Debug, Clone, Default)]
struct WifiState {
    iface: String,
    state: String,
    mac: String,
    ipv4: Vec<String>,
    ipv6: Vec<String>,
    current: Option<WifiNet>,
    networks: Vec<WifiNet>,
    error: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct BtAdapter {
    mac: String,
    name: String,
    powered: bool,
    discoverable: bool,
    pairable: bool,
}

#[derive(Debug, Clone)]
struct BtDevice {
    mac: String,
    name: String,
    connected: bool,
    paired: bool,
}

#[derive(Debug, Clone, Default)]
struct BtState {
    adapter: Option<BtAdapter>,
    paired: Vec<BtDevice>,
    connected: Vec<BtDevice>,
    known: Vec<BtDevice>,
    error: Option<String>,
}

// ---------------- collection ----------------

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

fn find_wifi_iface() -> Option<String> {
    let text = run("nmcli", &["-t", "-f", "DEVICE,TYPE", "dev"])?;
    for line in text.lines() {
        if let Some((dev, ty)) = line.split_once(':') {
            if ty == "wifi" {
                return Some(dev.to_string());
            }
        }
    }
    None
}

fn collect_wifi() -> WifiState {
    let mut st = WifiState::default();
    let iface = match find_wifi_iface() {
        Some(i) => i,
        None => {
            st.error = Some("no wifi interface found (nmcli)".into());
            return st;
        }
    };
    st.iface = iface.clone();

    if let Some(text) = run("ip", &["-j", "addr", "show", &iface]) {
        if let Ok(links) = serde_json::from_str::<Vec<IpLink>>(&text) {
            if let Some(link) = links
                .iter()
                .find(|l| l.ifname == iface)
                .or_else(|| links.first())
            {
                st.state = link.operstate.clone().unwrap_or_default().to_lowercase();
                st.mac = link.address.clone().unwrap_or_default();
                for a in &link.addr_info {
                    let s = format!("{}/{}", a.local, a.prefixlen);
                    match a.family.as_str() {
                        "inet" => st.ipv4.push(s),
                        "inet6" => {
                            if !a.local.starts_with("fe80") {
                                st.ipv6.push(s);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if let Some(records) = nmcli_multiline(&[
        "-f",
        "SSID,SIGNAL,SECURITY,FREQ,BSSID,RATE,IN-USE",
        "dev",
        "wifi",
        "list",
    ]) {
        for r in records {
            let ssid = r.get("SSID").cloned().unwrap_or_default();
            if ssid.is_empty() {
                continue;
            }
            let signal = r.get("SIGNAL").and_then(|s| s.parse().ok()).unwrap_or(0);
            let freq_mhz = parse_freq(r.get("FREQ").map(|s| s.as_str()).unwrap_or(""));
            let active = r.get("IN-USE").map(|v| v.contains('*')).unwrap_or(false);
            let net = WifiNet {
                ssid,
                signal,
                security: r.get("SECURITY").cloned().unwrap_or_default(),
                freq_mhz,
                bssid: r.get("BSSID").cloned().unwrap_or_default(),
                rate: r.get("RATE").cloned().unwrap_or_default(),
                active,
            };
            if active && st.current.is_none() {
                st.current = Some(net.clone());
            }
            st.networks.push(net);
        }
        st.networks.sort_by(|a, b| b.signal.cmp(&a.signal));
    }

    st
}

fn nmcli_multiline(args: &[&str]) -> Option<Vec<HashMap<String, String>>> {
    let out = Command::new("nmcli")
        .arg("-m")
        .arg("multiline")
        .args(args)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut records = Vec::new();
    let mut cur = HashMap::new();
    for line in text.lines() {
        if line.is_empty() {
            if !cur.is_empty() {
                records.push(std::mem::take(&mut cur));
            }
        } else if let Some((k, v)) = line.split_once(':') {
            cur.insert(k.trim().to_string(), v.to_string());
        }
    }
    if !cur.is_empty() {
        records.push(cur);
    }
    Some(records)
}

fn parse_freq(s: &str) -> u32 {
    s.split_whitespace()
        .next()
        .and_then(|x| x.parse().ok())
        .unwrap_or(0)
}

fn freq_band(mhz: u32) -> &'static str {
    if mhz == 0 {
        "?"
    } else if mhz < 3000 {
        "2.4GHz"
    } else if mhz < 5925 {
        "5GHz"
    } else {
        "6GHz"
    }
}

fn signal_bars(sig: u32) -> &'static str {
    match sig {
        0..=20 => "____",
        21..=40 => "▂___",
        41..=60 => "▂▄__",
        61..=80 => "▂▄▆_",
        _ => "▂▄▆█",
    }
}

fn collect_bt() -> BtState {
    let mut st = BtState::default();

    let show = run("bluetoothctl", &["show"]);
    if show.is_none() {
        st.error = Some("bluetoothctl failed or bluetooth service is not running".into());
    }
    if let Some(text) = show {
        let mut adapter = BtAdapter::default();
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("Controller ") {
                let rest = trimmed.strip_prefix("Controller ").unwrap_or("");
                adapter.mac = rest.split_whitespace().next().unwrap_or("").to_string();
            } else if let Some((k, v)) = trimmed.split_once(':') {
                let v = v.trim();
                match k.trim() {
                    "Name" | "Alias" => adapter.name = v.to_string(),
                    "Powered" => adapter.powered = v == "yes",
                    "Discoverable" => adapter.discoverable = v == "yes",
                    "Pairable" => adapter.pairable = v == "yes",
                    _ => {}
                }
            }
        }
        if !adapter.mac.is_empty() {
            st.adapter = Some(adapter);
        }
    }

    let parse_devices = |args: &[&str]| -> Vec<BtDevice> {
        let mut out = Vec::new();
        if let Some(text) = run("bluetoothctl", args) {
            for line in text.lines() {
                let rest = match line.strip_prefix("Device ") {
                    Some(r) => r,
                    None => continue,
                };
                let mut parts = rest.splitn(2, ' ');
                let mac = parts.next().unwrap_or("").to_string();
                let name = parts.next().unwrap_or("(unknown)").trim().to_string();
                if mac.is_empty() {
                    continue;
                }
                out.push(BtDevice {
                    mac,
                    name,
                    connected: false,
                    paired: false,
                });
            }
        }
        out
    };

    st.paired = parse_devices(&["devices", "Paired"]);
    st.known = parse_devices(&["devices"]);
    st.connected = parse_devices(&["devices", "Connected"]);

    let connected_macs: Vec<String> = st.connected.iter().map(|d| d.mac.clone()).collect();
    for d in &mut st.paired {
        d.paired = true;
        if connected_macs.contains(&d.mac) {
            d.connected = true;
        }
    }

    st
}

// ---------------- UI ----------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Wifi,
    Bluetooth,
}

impl Tab {
    fn index(self) -> usize {
        match self {
            Tab::Wifi => 0,
            Tab::Bluetooth => 1,
        }
    }
}

const TAB_LABELS: [&str; 2] = [" Wi-Fi ", " Bluetooth "];

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.hide_cursor()?;

    let res = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut tab = Tab::Wifi;
    let mut wifi = collect_wifi();
    let mut bt = collect_bt();
    let mut status = String::new();

    let scanning = Arc::new(AtomicBool::new(false));
    let mut was_scanning = false;

    loop {
        let is_scanning = scanning.load(Ordering::Relaxed);
        if was_scanning && !is_scanning {
            bt = collect_bt();
            status = "bluetooth scan complete".into();
        }
        was_scanning = is_scanning;

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ])
                .split(f.area());

            let tabs = Tabs::new(TAB_LABELS.iter().copied())
                .select(tab.index())
                .style(Style::default().fg(Color::Gray))
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .block(Block::default().borders(Borders::ALL).title(" asahi net "));
            f.render_widget(tabs, chunks[0]);

            match tab {
                Tab::Wifi => render_wifi(f, chunks[1], &wifi, &status),
                Tab::Bluetooth => render_bt(f, chunks[1], &bt, is_scanning, &status),
            }

            let help = Paragraph::new(Line::from(vec![
                Span::styled("1/2/tab", Style::default().fg(Color::Green)),
                Span::raw(" switch · "),
                Span::styled("r", Style::default().fg(Color::Green)),
                Span::raw(" refresh · "),
                Span::styled("s", Style::default().fg(Color::Green)),
                Span::raw(" scan · "),
                Span::styled("R", Style::default().fg(Color::Red)),
                Span::raw(" restart selected service · "),
                Span::styled("q", Style::default().fg(Color::Green)),
                Span::raw(" quit"),
            ]))
            .block(Block::default().borders(Borders::ALL).title(" keys "));
            f.render_widget(help, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('1') => tab = Tab::Wifi,
                    KeyCode::Char('2') => tab = Tab::Bluetooth,
                    KeyCode::Tab => {
                        tab = if tab == Tab::Wifi {
                            Tab::Bluetooth
                        } else {
                            Tab::Wifi
                        };
                    }
                    KeyCode::Char('r') => {
                        wifi = collect_wifi();
                        bt = collect_bt();
                        status = "refreshed".into();
                    }
                    KeyCode::Char('s') => match tab {
                        Tab::Wifi => {
                            wifi = collect_wifi();
                            status = "wifi rescanned".into();
                        }
                        Tab::Bluetooth => {
                            if !scanning.load(Ordering::Relaxed) {
                                scanning.store(true, Ordering::Relaxed);
                                let flag = scanning.clone();
                                std::thread::spawn(move || {
                                    let _ = Command::new("bluetoothctl")
                                        .args(["--timeout", "5", "scan", "on"])
                                        .output();
                                    flag.store(false, Ordering::Relaxed);
                                });
                                status = "scanning bluetooth (5s)...".into();
                            }
                        }
                    },
                    KeyCode::Char('R') => match tab {
                        Tab::Wifi => {
                            status = restart_modules(&[("brcmfmac", "brcmfmac")]);
                            wifi = collect_wifi();
                        }
                        Tab::Bluetooth => {
                            status = restart_modules(&[("hci_bcm4377", "hci_bcm4377")]);
                            bt = collect_bt();
                        }
                    },
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn render_wifi(f: &mut ratatui::Frame, area: Rect, wifi: &WifiState, status: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(5)])
        .split(area);

    let header_lines = if let Some(err) = &wifi.error {
        vec![Line::from(Span::styled(
            err.clone(),
            Style::default().fg(Color::Red),
        ))]
    } else {
        let state_color = match wifi.state.as_str() {
            "up" => Color::Green,
            "down" => Color::Red,
            _ => Color::Yellow,
        };
        let mut lines = vec![
            Line::from(vec![
                Span::raw("interface: "),
                Span::styled(&wifi.iface, Style::default().fg(Color::Cyan)),
                Span::raw("  state: "),
                Span::styled(&wifi.state, Style::default().fg(state_color)),
                Span::raw("  mac: "),
                Span::raw(&wifi.mac),
            ]),
            Line::from(format!(
                "ipv4:  {}",
                if wifi.ipv4.is_empty() {
                    "(none)".into()
                } else {
                    wifi.ipv4.join(", ")
                }
            )),
            Line::from(format!(
                "ipv6:  {}",
                if wifi.ipv6.is_empty() {
                    "(none)".into()
                } else {
                    wifi.ipv6.join(", ")
                }
            )),
        ];
        if let Some(cur) = &wifi.current {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::raw("ssid: "),
                Span::styled(
                    &cur.ssid,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("   bssid: "),
                Span::raw(&cur.bssid),
            ]));
            lines.push(Line::from(vec![
                Span::raw("signal: "),
                Span::styled(
                    format!("{} {}%", signal_bars(cur.signal), cur.signal),
                    Style::default().fg(signal_color(cur.signal)),
                ),
                Span::raw("   freq: "),
                Span::raw(format!(
                    "{} MHz ({})",
                    cur.freq_mhz,
                    freq_band(cur.freq_mhz)
                )),
            ]));
            lines.push(Line::from(vec![
                Span::raw("security: "),
                Span::raw(&cur.security),
                Span::raw("   rate: "),
                Span::raw(&cur.rate),
            ]));
        }
        lines
    };

    let header = Paragraph::new(header_lines)
        .block(Block::default().borders(Borders::ALL).title(" connection "));
    f.render_widget(header, chunks[0]);

    let mut list: Vec<Line> = Vec::new();
    if wifi.networks.is_empty() {
        list.push(Line::from(Span::styled(
            "  no networks visible",
            Style::default().fg(Color::Gray),
        )));
    } else {
        list.push(Line::from(Span::styled(
            format!(
                "  {:<2} {:<28} {:>5} {:>4}  {:<5} {:<16}",
                "", "ssid", "bars", "sig", "band", "security"
            ),
            Style::default().fg(Color::DarkGray),
        )));
        for n in &wifi.networks {
            let marker = if n.active { "▶★" } else { "  " };
            let ssid_color = if n.active { Color::Cyan } else { Color::White };
            list.push(Line::from(vec![
                Span::raw(format!("  {} ", marker)),
                Span::styled(
                    format!("{:<28}", trunc(&n.ssid, 28)),
                    Style::default().fg(ssid_color),
                ),
                Span::raw(format!(" {:>5} {:>3}%", signal_bars(n.signal), n.signal)),
                Span::raw(format!("  {:<5}", freq_band(n.freq_mhz))),
                Span::raw(format!(" {}", trunc(&n.security, 20))),
            ]));
        }
    }
    list.push(Line::from(""));
    list.push(Line::from(Span::styled(
        format!("  {}", status),
        Style::default().fg(Color::Gray),
    )));

    let list_w = Paragraph::new(list).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" nearby networks ({}) ", wifi.networks.len())),
    );
    f.render_widget(list_w, chunks[1]);
}

fn render_bt(f: &mut ratatui::Frame, area: Rect, bt: &BtState, scanning: bool, status: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(5)])
        .split(area);

    let adapter_lines = if let Some(a) = &bt.adapter {
        vec![
            Line::from(vec![
                Span::raw("adapter: "),
                Span::styled(&a.mac, Style::default().fg(Color::Cyan)),
                Span::raw("  name: "),
                Span::raw(&a.name),
            ]),
            Line::from(vec![
                Span::raw("powered: "),
                Span::styled(
                    if a.powered { "yes" } else { "no" },
                    Style::default().fg(if a.powered { Color::Green } else { Color::Red }),
                ),
                Span::raw("   discoverable: "),
                Span::raw(if a.discoverable { "yes" } else { "no" }),
                Span::raw("   pairable: "),
                Span::raw(if a.pairable { "yes" } else { "no" }),
            ]),
        ]
    } else {
        let msg = bt
            .error
            .clone()
            .unwrap_or_else(|| "no bluetooth adapter found".into());
        vec![Line::from(Span::styled(
            msg,
            Style::default().fg(Color::Red),
        ))]
    };
    let adapter_w = Paragraph::new(adapter_lines)
        .block(Block::default().borders(Borders::ALL).title(" adapter "));
    f.render_widget(adapter_w, chunks[0]);

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        "  connected",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));
    if bt.connected.is_empty() {
        lines.push(Line::from(Span::styled(
            "    (none)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for d in &bt.connected {
            lines.push(Line::from(vec![
                Span::raw("    ● "),
                Span::styled(&d.name, Style::default().fg(Color::Cyan)),
                Span::raw(format!("  {}", d.mac)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("  paired ({})", bt.paired.len()),
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    )));
    if bt.paired.is_empty() {
        lines.push(Line::from(Span::styled(
            "    (none)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for d in &bt.paired {
            let marker = if d.connected { "●" } else { "○" };
            lines.push(Line::from(format!("    {} {}  {}", marker, d.name, d.mac)));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("  known devices ({})", bt.known.len()),
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    )));
    for d in &bt.known {
        if bt.paired.iter().any(|p| p.mac == d.mac) {
            continue;
        }
        lines.push(Line::from(format!("    · {}  {}", d.name, d.mac)));
    }

    lines.push(Line::from(""));
    if scanning {
        lines.push(Line::from(Span::styled(
            "  scanning... (press s again to refresh after 5s)",
            Style::default().fg(Color::Yellow),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("  {}", status),
            Style::default().fg(Color::Gray),
        )));
        lines.push(Line::from(Span::styled(
            "  press s to scan for nearby devices (5s)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let list = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(" devices "));
    f.render_widget(list, chunks[1]);
}

fn signal_color(sig: u32) -> Color {
    if sig >= 70 {
        Color::Green
    } else if sig >= 40 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n.saturating_sub(1)).collect::<String>() + "…"
    }
}

fn restart_modules(pairs: &[(&str, &str)]) -> String {
    for (unload, load) in pairs {
        let _ = Command::new("sudo")
            .args(["-n", "modprobe", "-r", unload])
            .status();
        std::thread::sleep(std::time::Duration::from_millis(400));
        let r = Command::new("sudo").args(["-n", "modprobe", load]).status();
        match r {
            Ok(s) if s.success() => {}
            _ => {
                let _ = Command::new("pkexec").args(["modprobe", load]).status();
            }
        }
    }
    "modules restarted".into()
}
