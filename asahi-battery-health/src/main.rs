use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::Local;
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
    widgets::{Block, Borders, Paragraph, Sparkline},
    Terminal,
};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::io::IsTerminal;

const BAT: &str = "/sys/class/power_supply/macsmc-battery";

fn read_i64(attr: &str) -> Option<i64> {
    std::fs::read_to_string(format!("{BAT}/{attr}"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn read_str(attr: &str) -> Option<String> {
    std::fs::read_to_string(format!("{BAT}/{attr}"))
        .ok()
        .map(|s| s.trim().to_string())
}

#[derive(Clone, Debug, Serialize)]
struct Sample {
    capacity: Option<i64>,
    health: Option<String>,
    cycles: Option<i64>,
    temp_c: Option<f64>,
    voltage_v: Option<f64>,
    power_w: Option<f64>,
    status: Option<String>,
}

fn sample() -> Sample {
    Sample {
        capacity: read_i64("capacity"),
        health: read_str("health"),
        cycles: read_i64("cycle_count"),
        temp_c: read_i64("temp").map(|t| t as f64 / 10.0),
        voltage_v: read_i64("voltage_now").map(|v| v as f64 / 1_000_000.0),
        power_w: read_i64("power_now").map(|p| p as f64 / 1_000_000.0),
        status: read_str("status"),
    }
}

fn db_path() -> PathBuf {
    let dir = dirs_or_home().join(".local/share/asahi-tools");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("battery-health.sqlite")
}

fn dirs_or_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn init_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS samples (
            ts TEXT PRIMARY KEY,
            capacity INTEGER,
            health TEXT,
            cycles INTEGER,
            temp_c REAL,
            voltage_v REAL,
            power_w REAL,
            status TEXT
        );",
    )
}

fn insert(conn: &Connection, s: &Sample) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO samples
         (ts, capacity, health, cycles, temp_c, voltage_v, power_w, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            Local::now().to_rfc3339(),
            s.capacity,
            s.health,
            s.cycles,
            s.temp_c,
            s.voltage_v,
            s.power_w,
            s.status
        ],
    )?;
    Ok(())
}

fn load_series(conn: &Connection, limit: usize) -> Vec<(String, Option<i64>)> {
    let mut stmt = match conn.prepare("SELECT ts, capacity FROM samples ORDER BY ts DESC LIMIT ?1")
    {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map([limit as i64], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
    });
    let mut out = Vec::new();
    if let Ok(rows) = rows {
        for row in rows.flatten() {
            out.push(row);
        }
    }
    out.reverse();
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open(db_path())?;
    init_db(&conn)?;

    // If stdin is not a TTY (e.g. launched from a timer), log once and exit.
    if !io::stdout().is_terminal() {
        let s = sample();
        insert(&conn, &s).ok();
        println!("{}", serde_json::to_string(&s).unwrap_or_default());
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.hide_cursor()?;
    let res = run(&mut terminal, &conn);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, conn: &Connection) -> io::Result<()> {
    let mut current = sample();
    let mut series = load_series(conn, 120);
    let mut last_log = Instant::now() - Duration::from_secs(3600);

    loop {
        if last_log.elapsed() > Duration::from_secs(60) {
            insert(conn, &current).ok();
            series = load_series(conn, 120);
            last_log = Instant::now();
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Min(6),
                    Constraint::Length(3),
                ])
                .split(f.area());

            let cap = current.capacity.unwrap_or(-1);
            let cap_style = if cap < 20 {
                Style::default().fg(Color::Red)
            } else if cap < 40 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            };
            let title = Paragraph::new(vec![
                Line::from(vec![
                    Span::raw("capacity: "),
                    Span::styled(format!("{cap}%"), cap_style.add_modifier(Modifier::BOLD)),
                    Span::raw("   status: "),
                    Span::raw(current.status.clone().unwrap_or_else(|| "?".into())),
                ]),
                Line::from(vec![
                    Span::raw("health: "),
                    Span::raw(current.health.clone().unwrap_or_else(|| "?".into())),
                    Span::raw("   cycles: "),
                    Span::raw(
                        current
                            .cycles
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "?".into()),
                    ),
                ]),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" macsmc-battery (raw sysfs) "),
            );
            f.render_widget(title, chunks[0]);

            let temp = current.temp_c.unwrap_or(0.0);
            let volt = current.voltage_v.unwrap_or(0.0);
            let pow = current.power_w.unwrap_or(0.0);
            let metrics = Paragraph::new(vec![
                Line::from(format!("temp:     {temp:>6.1} °C")),
                Line::from(format!("voltage:  {volt:>6.2} V")),
                Line::from(format!("power:    {pow:>6.2} W")),
            ])
            .block(Block::default().borders(Borders::ALL).title(" electrical "));
            f.render_widget(metrics, chunks[1]);

            let last_ts = series
                .last()
                .map(|(ts, _)| ts.clone())
                .unwrap_or_else(|| "-".into());
            let meta = Paragraph::new(vec![
                Line::from(format!("samples: {}", series.len())),
                Line::from(format!("last:    {last_ts}")),
                Line::from("logged to ~/.local/share/asahi-tools/battery-health.sqlite"),
            ])
            .block(Block::default().borders(Borders::ALL).title(" log "));
            f.render_widget(meta, chunks[2]);

            let data: Vec<u64> = series
                .iter()
                .filter_map(|(_, c)| c.map(|v| v.clamp(0, 100) as u64))
                .collect();
            let spark = Sparkline::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" capacity history (last 120 samples) "),
                )
                .data(&data[..])
                .style(Style::default().fg(Color::Cyan));
            f.render_widget(spark, chunks[3]);

            let help = Paragraph::new(Line::from(vec![
                Span::styled("q", Style::default().fg(Color::Green)),
                Span::raw(" quit · "),
                Span::styled("r", Style::default().fg(Color::Green)),
                Span::raw(" force log now · samples auto-log every 60s"),
            ]))
            .block(Block::default().borders(Borders::ALL).title(" keys "));
            f.render_widget(help, chunks[4]);
        })?;

        if event::poll(Duration::from_millis(500))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('r') => {
                        current = sample();
                        insert(conn, &current).ok();
                        series = load_series(conn, 120);
                        last_log = Instant::now();
                    }
                    _ => {}
                }
            }
        }

        current = sample();
    }
    Ok(())
}
