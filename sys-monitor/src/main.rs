use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

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
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use serde::Deserialize;

const SOCKET_PATH: &str = "/run/asahi-sysmon.sock";

#[derive(Deserialize, Debug, Clone)]
struct FanView {
    index: u32,
    rpm: u32,
    min: Option<u32>,
    max: Option<u32>,
    target: Option<u32>,
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)]
struct Status {
    ok: bool,
    readonly: bool,
    max_temp_c: f64,
    temps: Vec<(String, f64)>,
    fans: Vec<FanView>,
    #[serde(default)]
    message: Option<String>,
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut status: Option<Status> = None;
    let mut selected: usize = 0;

    loop {
        // refresh
        let err: Option<String> = match query(&serde_json::json!({"cmd": "status"})) {
            Ok(v) => match serde_json::from_value::<Status>(v) {
                Ok(s) => {
                    status = Some(s);
                    None
                }
                Err(e) => Some(format!("decode: {e}")),
            },
            Err(e) => Some(format!("connect: {e}")),
        };

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(12),
                    Constraint::Length(7),
                    Constraint::Length(3),
                ])
                .split(f.area());

            let max_temp = status.as_ref().map(|s| s.max_temp_c).unwrap_or(0.0);
            let header = Paragraph::new(format!("max temp: {:.1} °C", max_temp))
                .style(
                    Style::default()
                        .fg(if max_temp > 80.0 {
                            Color::Red
                        } else if max_temp > 70.0 {
                            Color::Yellow
                        } else {
                            Color::Green
                        })
                        .add_modifier(Modifier::BOLD),
                )
                .block(Block::default().borders(Borders::ALL).title(" sysmon "));
            f.render_widget(header, chunks[0]);

            let mut temp_lines: Vec<Line> = Vec::new();
            if let Some(s) = &status {
                for (name, val) in &s.temps {
                    temp_lines.push(Line::from(format!("  {name:>10}: {val:6.1} °C")));
                }
            } else if let Some(e) = &err {
                temp_lines.push(Line::from(Span::styled(
                    format!("  {e}"),
                    Style::default().fg(Color::Red),
                )));
                temp_lines.push(Line::from(Span::styled(
                    "  is the daemon running? sudo ~/asahi-tools/target/release/sysmon-daemon",
                    Style::default().fg(Color::Gray),
                )));
            }
            let temps = Paragraph::new(temp_lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" temperatures "),
            );
            f.render_widget(temps, chunks[1]);

            let mut fan_lines: Vec<Line> = Vec::new();
            if let Some(s) = &status {
                if s.readonly {
                    fan_lines.push(Line::from(Span::styled(
                        "  fan control: read-only",
                        Style::default().fg(Color::Yellow),
                    )));
                }
                for (i, fv) in s.fans.iter().enumerate() {
                    let sel = if i == selected { "▶ " } else { "  " };
                    let target = fv
                        .target
                        .map(|t| format!("target {t}"))
                        .unwrap_or_else(|| "auto".to_string());
                    let range = format!(
                        "[{}-{}]",
                        fv.min.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
                        fv.max.map(|v| v.to_string()).unwrap_or_else(|| "?".into())
                    );
                    let style = if i == selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    fan_lines.push(Line::from(Span::styled(
                        format!(
                            "  {sel}fan{}  {:>5} rpm   {range}   {target}",
                            fv.index, fv.rpm
                        ),
                        style,
                    )));
                }
            }
            let fans = Paragraph::new(fan_lines)
                .block(Block::default().borders(Borders::ALL).title(" fans "));
            f.render_widget(fans, chunks[2]);

            let help = Paragraph::new(Line::from(vec![
                Span::styled("↑/↓", Style::default().fg(Color::Green)),
                Span::raw(" select fan · "),
                Span::styled("+/-", Style::default().fg(Color::Green)),
                Span::raw(" ±250 rpm · "),
                Span::styled("m", Style::default().fg(Color::Green)),
                Span::raw(" max · "),
                Span::styled("a", Style::default().fg(Color::Green)),
                Span::raw(" auto · "),
                Span::styled("q", Style::default().fg(Color::Green)),
                Span::raw(" quit"),
            ]))
            .block(Block::default().borders(Borders::ALL).title(" keys "));
            f.render_widget(help, chunks[3]);
        })?;

        if event::poll(std::time::Duration::from_millis(1000))? {
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
                        if let Some(s) = &status {
                            if selected + 1 < s.fans.len() {
                                selected += 1;
                            }
                        }
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        if let Some(s) = &status {
                            if let Some(fv) = s.fans.get(selected) {
                                let cur = fv.target.unwrap_or(fv.rpm);
                                let _ = query(&serde_json::json!({
                                    "cmd": "set_fan",
                                    "fan": fv.index,
                                    "target": cur + 250
                                }));
                            }
                        }
                    }
                    KeyCode::Char('-') => {
                        if let Some(s) = &status {
                            if let Some(fv) = s.fans.get(selected) {
                                let cur = fv.target.unwrap_or(fv.rpm);
                                let _ = query(&serde_json::json!({
                                    "cmd": "set_fan",
                                    "fan": fv.index,
                                    "target": cur.saturating_sub(250)
                                }));
                            }
                        }
                    }
                    KeyCode::Char('m') => {
                        if let Some(s) = &status {
                            if let Some(fv) = s.fans.get(selected) {
                                let _ = query(&serde_json::json!({
                                    "cmd": "max",
                                    "fan": fv.index
                                }));
                            }
                        }
                    }
                    KeyCode::Char('a') => {
                        if let Some(s) = &status {
                            if let Some(fv) = s.fans.get(selected) {
                                let _ = query(&serde_json::json!({
                                    "cmd": "auto",
                                    "fan": fv.index
                                }));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn query(req: &serde_json::Value) -> io::Result<serde_json::Value> {
    let mut stream = UnixStream::connect(SOCKET_PATH)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    writeln!(stream, "{req}")?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    serde_json::from_str(&line).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
