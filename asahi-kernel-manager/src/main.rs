use std::io::{self, BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

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
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};

#[derive(Default, Clone)]
struct Pkg {
    name: String,
    version: String,
}

#[derive(Default, Clone)]
struct Snapshot {
    current_kernel: String,
    uptime: String,
    installed_kernels: Vec<Pkg>,
    available_updates: Vec<Pkg>,
    firmware_version: String,
    m1n1_status: String,
    last_check: Option<String>,
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() && cmd != "dnf" {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

fn read_local() -> Snapshot {
    let mut s = Snapshot::default();

    s.current_kernel = run("uname", &["-r"]).unwrap_or_default().trim().to_string();

    // Uptime — defensive
    if let Ok(txt) = std::fs::read_to_string("/proc/uptime") {
        if let Some(first) = txt.split_whitespace().next() {
            if let Ok(secs) = first.parse::<u64>() {
                s.uptime = fmt_uptime(secs);
            }
        }
    }
    if s.uptime.is_empty() {
        s.uptime = run("uptime", &["-p"])
            .map(|t| t.trim().trim_start_matches("up ").to_string())
            .unwrap_or_else(|| "-".into());
    }

    // Installed kernels: match all kernel* packages
    if let Some(out) = run(
        "rpm",
        &["-qa", "--qf", "%{NAME} %{VERSION}-%{RELEASE}\n", "kernel*"],
    ) {
        s.installed_kernels = out
            .lines()
            .filter(|l| !l.is_empty() && !l.contains("not installed"))
            .filter_map(|l| {
                let mut it = l.splitn(2, ' ');
                let name = it.next()?.to_string();
                let version = it.next()?.to_string();
                name.starts_with("kernel").then_some(Pkg { name, version })
            })
            .collect();
    }

    // Firmware — try multiple paths
    for path in [
        "/proc/device-tree/chosen/firmware-version",
        "/proc/device-tree/chosen/asahi,firmware-version",
        "/sys/firmware/devicetree/base/chosen/firmware-version",
        "/proc/device-tree/chosen/asahi,os-firmware-version",
    ] {
        if let Ok(t) = std::fs::read_to_string(path) {
            let clean = t.trim_matches('\0').trim().to_string();
            if !clean.is_empty() {
                s.firmware_version = clean;
                break;
            }
        }
    }
    if s.firmware_version.is_empty() {
        s.firmware_version = "unknown".into();
    }

    // m1n1 / U-Boot — try multiple paths
    let mut uboot = String::new();
    for path in [
        "/proc/device-tree/chosen/asahi,uboot-version",
        "/proc/device-tree/chosen/u-boot,version",
        "/sys/firmware/devicetree/base/chosen/asahi,uboot-version",
        "/proc/device-tree/chosen/asahi,uboot-version-string",
    ] {
        if let Ok(t) = std::fs::read_to_string(path) {
            let clean = t.trim_matches('\0').trim().to_string();
            if !clean.is_empty() {
                uboot = clean;
                break;
            }
        }
    }
    if uboot.is_empty() {
        uboot = run("m1n1-version", &[]).unwrap_or_default().trim().to_string();
    }
    s.m1n1_status = if uboot.is_empty() {
        "unknown".into()
    } else {
        format!("u-boot {}", uboot)
    };

    s
}

fn check_updates(snap: &mut Snapshot) -> String {
    match Command::new("dnf")
        .args(["check-update", "kernel", "--quiet"])
        .output()
    {
        Ok(out) => {
            // dnf exit codes: 0 = no updates, 100 = updates available
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            let mut updates = Vec::new();
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with("Last metadata") {
                    continue;
                }
                let mut it = trimmed.split_whitespace();
                if let (Some(name), Some(version)) = (it.next(), it.next()) {
                    if name.starts_with("kernel") {
                        updates.push(Pkg {
                            name: name.to_string(),
                            version: version.to_string(),
                        });
                    }
                }
            }
            snap.available_updates = updates;
            snap.last_check = Some(chrono_free_time());
            if snap.available_updates.is_empty() {
                "check complete — up to date".to_string()
            } else {
                format!("check complete — {} update(s)", snap.available_updates.len())
            }
        }
        Err(e) => format!("check failed: {e}"),
    }
}

fn chrono_free_time() -> String {
    // Avoid chrono dep — use date command
    Command::new("date")
        .arg("+%H:%M:%S")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn fmt_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 { format!("{d}d {h}h") }
    else if h > 0 { format!("{h}h {m}m") }
    else { format!("{m}m") }
}

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
    let mut status = String::from("press c to check for kernel updates");
    let mut status_color = Color::DarkGray;
    let mut snap = read_local();
    let mut confirm: Option<&'static str> = None;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6),
                    Constraint::Min(8),
                    Constraint::Length(9),
                    Constraint::Length(3),
                ])
                .split(f.area());

            // Header
            let header = vec![
                Line::from(vec![
                    Span::styled("Current kernel  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&snap.current_kernel, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("Uptime          ", Style::default().fg(Color::DarkGray)),
                    Span::raw(&snap.uptime),
                ]),
                Line::from(vec![
                    Span::styled("m1n1            ", Style::default().fg(Color::DarkGray)),
                    Span::raw(&snap.m1n1_status),
                ]),
                Line::from(vec![
                    Span::styled("Firmware        ", Style::default().fg(Color::DarkGray)),
                    Span::raw(&snap.firmware_version),
                ]),
            ];
            f.render_widget(
                Paragraph::new(header).block(Block::default().borders(Borders::ALL).title(" System ")),
                chunks[0],
            );

            // Kernels: installed vs available
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);

            let mut installed: Vec<Line> = Vec::new();
            if snap.installed_kernels.is_empty() {
                installed.push(Line::from(Span::styled("(none found)", Style::default().fg(Color::DarkGray))));
            } else {
                for p in &snap.installed_kernels {
                    installed.push(Line::from(vec![
                        Span::styled(&p.name, Style::default().fg(Color::Cyan)),
                        Span::raw("  "),
                        Span::raw(&p.version),
                    ]));
                }
            }
            f.render_widget(
                Paragraph::new(installed)
                    .block(Block::default().borders(Borders::ALL).title(format!(
                        " Installed kernels ({}) ",
                        snap.installed_kernels.len()
                    ))),
                cols[0],
            );

            let mut updates: Vec<Line> = Vec::new();
            if snap.last_check.is_none() {
                updates.push(Line::from(Span::styled(
                    "press c to check for updates",
                    Style::default().fg(Color::DarkGray),
                )));
            } else if snap.available_updates.is_empty() {
                updates.push(Line::from(Span::styled("✓ up to date", Style::default().fg(Color::Green))));
            } else {
                updates.push(Line::from(Span::styled(
                    format!("{} update(s) available", snap.available_updates.len()),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )));
                updates.push(Line::from(""));
                for p in &snap.available_updates {
                    updates.push(Line::from(vec![
                        Span::styled(&p.name, Style::default().fg(Color::Yellow)),
                        Span::raw("  "),
                        Span::raw(&p.version),
                    ]));
                }
            }
            f.render_widget(
                Paragraph::new(updates)
                    .block(Block::default().borders(Borders::ALL).title(" Updates ")),
                cols[1],
            );

            // Actions
            let actions = vec![
                Line::from(vec![
                    Span::styled("c", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw("  check for updates"),
                ]),
                Line::from(vec![
                    Span::styled("u", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw("  upgrade kernel-asahi* (dnf upgrade + update-m1n1)"),
                ]),
                Line::from(vec![
                    Span::styled("r", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw("  run update-m1n1 (refresh bootloader)"),
                ]),
                Line::from(vec![
                    Span::styled("f", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw("  run asahi-fwupdate (firmware)"),
                ]),
                Line::from(vec![
                    Span::styled("l", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw("  show system journal errors from last boot"),
                ]),
                Line::from(""),
                if let Some(c) = confirm {
                    Line::from(Span::styled(
                        format!("⚠ press ENTER to confirm `{c}`, ESC to cancel"),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ))
                } else {
                    Line::from(Span::styled(
                        "q  quit",
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ))
                },
            ];
            f.render_widget(
                Paragraph::new(actions)
                    .wrap(Wrap { trim: false })
                    .block(Block::default().borders(Borders::ALL).title(" Actions ")),
                chunks[2],
            );

            // Status
            f.render_widget(
                Paragraph::new(Span::styled(status.as_str(), Style::default().fg(status_color)))
                    .block(Block::default().borders(Borders::ALL).title(" Status ")),
                chunks[3],
            );
        })?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                // Confirm pending
                if let Some(cmd) = confirm {
                    match key.code {
                        KeyCode::Enter => {
                            let result = execute_action(terminal, cmd)?;
                            status = result.0;
                            status_color = result.1;
                            confirm = None;
                            snap = read_local();
                        }
                        KeyCode::Esc | KeyCode::Char('q') => {
                            confirm = None;
                            status = "cancelled".into();
                            status_color = Color::DarkGray;
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c') => {
                        let msg = check_updates(&mut snap);
                        status_color = if snap.available_updates.is_empty() {
                            Color::Green
                        } else {
                            Color::Yellow
                        };
                        status = msg;
                    }
                    KeyCode::Char('u') => {
                        if snap.available_updates.is_empty() {
                            status = "no kernel updates available (press c to refresh)".into();
                            status_color = Color::DarkGray;
                        } else {
                            confirm = Some("upgrade");
                            status = "confirm kernel upgrade".into();
                            status_color = Color::Yellow;
                        }
                    }
                    KeyCode::Char('r') => {
                        confirm = Some("update-m1n1");
                        status = "confirm update-m1n1".into();
                        status_color = Color::Yellow;
                    }
                    KeyCode::Char('f') => {
                        confirm = Some("fwupdate");
                        status = "confirm asahi-fwupdate".into();
                        status_color = Color::Yellow;
                    }
                    KeyCode::Char('l') => {
                        let (msg, col) = execute_action(terminal, "journal")?;
                        status = msg;
                        status_color = col;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn execute_action(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    action: &str,
) -> io::Result<(String, Color)> {
    let (success, summary) = match action {
        "upgrade" => run_interactive(
            terminal,
            "sudo",
            &["dnf", "upgrade", "-y", "kernel-asahi*"],
        )?,
        "update-m1n1" => run_interactive(terminal, "sudo", &["update-m1n1"])?,
        "fwupdate" => run_interactive(terminal, "sudo", &["asahi-fwupdate"])?,
        "journal" => run_interactive(terminal, "journalctl", &["-p", "err", "-b", "-n", "100", "--no-pager"])?,
        _ => (false, "unknown action".into()),
    };

    // After upgrade, automatically run update-m1n1
    if action == "upgrade" && success {
        let _ = run_interactive(terminal, "sudo", &["update-m1n1"])?;
    }

    Ok(if success {
        (summary, Color::Green)
    } else {
        (summary, Color::Red)
    })
}

/// Suspend the TUI, run a command with inherited stdio, wait for Enter, restore.
fn run_interactive(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    program: &str,
    args: &[&str],
) -> io::Result<(bool, String)> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    println!();
    println!("$ {} {}", program, args.join(" "));
    println!();

    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

    let success = status.as_ref().map(|s| s.success()).unwrap_or(false);
    let code = status
        .as_ref()
        .ok()
        .map(|s| s.code().unwrap_or(-1))
        .unwrap_or(-1);

    println!();
    println!("[exit code {} — press Enter to return]", code);

    // Wait for Enter
    let mut line = String::new();
    let _ = BufReader::new(io::stdin()).read_line(&mut line);

    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;
    terminal.hide_cursor()?;

    // Drain any buffered events
    for _ in 0..256 {
        if event::poll(Duration::ZERO)?.then_some(()).is_none() {
            break;
        }
        let _ = event::read();
    }

    Ok((
        success,
        format!("{program} {} — exit {}", args.join(" "), code),
    ))
}
