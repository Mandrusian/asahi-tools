use std::io;

use asahi_core::{get_kbd_brightness, get_kbd_max_brightness, set_kbd_brightness};
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
    let max = get_kbd_max_brightness().unwrap_or(255);
    let mut brightness = get_kbd_brightness().unwrap_or(0);
    let mut status = String::new();

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(1),
                ])
                .split(f.area());

            let title = Paragraph::new(format!("Keyboard backlight: {brightness} / {max}"))
                .style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .block(Block::default().borders(Borders::ALL).title(" asahi kbd "));
            f.render_widget(title, chunks[0]);

            let ratio = if max == 0 {
                0.0
            } else {
                brightness as f64 / max as f64
            };
            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title(" brightness "))
                .gauge_style(Style::default().fg(Color::Yellow))
                .ratio(ratio.clamp(0.0, 1.0))
                .label(format!("{:.0}%", ratio * 100.0));
            f.render_widget(gauge, chunks[1]);

            let info = Paragraph::new(status.as_str())
                .style(Style::default().fg(Color::Gray))
                .block(Block::default().borders(Borders::ALL).title(" status "));
            f.render_widget(info, chunks[2]);

            let help = Paragraph::new(Line::from(vec![
                Span::styled("↑/↓", Style::default().fg(Color::Green)),
                Span::raw(" or "),
                Span::styled("+/-", Style::default().fg(Color::Green)),
                Span::raw(" adjust · "),
                Span::styled("m", Style::default().fg(Color::Green)),
                Span::raw(" max · "),
                Span::styled("0", Style::default().fg(Color::Green)),
                Span::raw(" off · "),
                Span::styled("q", Style::default().fg(Color::Green)),
                Span::raw(" quit"),
            ]))
            .block(Block::default().borders(Borders::ALL).title(" keys "));
            f.render_widget(help, chunks[3]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let step = (max / 20).max(1);
                let mut new_val = brightness;
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Up | KeyCode::Char('+') => new_val = (brightness + step).min(max),
                    KeyCode::Down | KeyCode::Char('-') => new_val = brightness.saturating_sub(step),
                    KeyCode::Char('m') => new_val = max,
                    KeyCode::Char('0') => new_val = 0,
                    _ => {}
                }
                if new_val != brightness {
                    match set_kbd_brightness(new_val) {
                        Ok(()) => {
                            brightness = new_val;
                            status = format!("wrote {new_val} to sysfs");
                        }
                        Err(e) => {
                            status = format!("write failed: {e} (run with sudo)");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
