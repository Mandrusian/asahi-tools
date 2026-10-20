use std::io;

use asahi_core::{
    get_battery_capacity, get_battery_status, get_charge_thresholds, set_charge_thresholds,
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
    widgets::{Block, Borders, Gauge, Paragraph},
    Terminal,
};

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
    let (mut start, mut end) = get_charge_thresholds().unwrap_or((0, 100));
    let mut focus_start = false;
    let mut status = String::new();

    loop {
        let capacity = get_battery_capacity().unwrap_or(0);
        let charge_status = get_battery_status().unwrap_or_else(|_| "unknown".into());

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(5),
                    Constraint::Length(3),
                    Constraint::Min(1),
                ])
                .split(f.area());

            let header = Paragraph::new(format!("Battery: {capacity}%  ({charge_status})"))
                .style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" asahi battery "),
                );
            f.render_widget(header, chunks[0]);

            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title(" charge "))
                .gauge_style(Style::default().fg(Color::Green))
                .ratio((capacity.min(100) as f64) / 100.0)
                .label(format!("{capacity}%"));
            f.render_widget(gauge, chunks[1]);

            let threshold_lines = vec![
                Line::from(vec![
                    Span::raw("start threshold: "),
                    Span::styled(
                        format!("{start:>3}%"),
                        if !focus_start {
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        },
                    ),
                    if !focus_start {
                        Span::raw("   ← editing")
                    } else {
                        Span::raw("")
                    },
                ]),
                Line::from(vec![
                    Span::raw("end   threshold: "),
                    Span::styled(
                        format!("{end:>3}%"),
                        if focus_start {
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        },
                    ),
                    if focus_start {
                        Span::raw("   ← editing")
                    } else {
                        Span::raw("")
                    },
                ]),
            ];
            let thresholds = Paragraph::new(threshold_lines)
                .block(Block::default().borders(Borders::ALL).title(" thresholds "));
            f.render_widget(thresholds, chunks[2]);

            let status_w = Paragraph::new(status.as_str())
                .style(Style::default().fg(Color::Gray))
                .block(Block::default().borders(Borders::ALL).title(" status "));
            f.render_widget(status_w, chunks[3]);

            let help = Paragraph::new(Line::from(vec![
                Span::styled("tab", Style::default().fg(Color::Green)),
                Span::raw(" switch · "),
                Span::styled("↑/↓", Style::default().fg(Color::Green)),
                Span::raw(" ±5 · "),
                Span::styled("s", Style::default().fg(Color::Green)),
                Span::raw(" save (needs root) · "),
                Span::styled("q", Style::default().fg(Color::Green)),
                Span::raw(" quit"),
            ]))
            .block(Block::default().borders(Borders::ALL).title(" keys "));
            f.render_widget(help, chunks[4]);
        })?;

        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab => focus_start = !focus_start,
                    KeyCode::Up => {
                        if focus_start && start + 5 < end {
                            start += 5;
                        } else if !focus_start && end < 100 {
                            end += 5;
                        }
                    }
                    KeyCode::Down => {
                        if focus_start && start >= 5 {
                            start -= 5;
                        } else if !focus_start && end >= start + 5 {
                            end -= 5;
                        }
                    }
                    KeyCode::Char('s') => match set_charge_thresholds(start, end) {
                        Ok(()) => status = format!("saved: start={start}% end={end}%"),
                        Err(e) => status = format!("save failed: {e} (run with sudo)"),
                    },
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
