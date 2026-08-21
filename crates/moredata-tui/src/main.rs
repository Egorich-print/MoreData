//! Diagnostic TUI. Talks to status types only — never calls process().

use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use moredata_audio::probe;
use moredata_core::StatusReport;
use moredata_plugin::builtin_catalog;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use std::io::{Result, stdout};
use std::time::Duration;

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let status = StatusReport::current();
    let audio = probe();
    let plugins = builtin_catalog();

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(7),
                    Constraint::Length(8),
                    Constraint::Min(4),
                ])
                .split(f.area());
            let s = format!(
                "engine {} v{}  pd_coupled={}  rt={}  ctrl={}\nq: quit",
                status.engine,
                status.version,
                status.pd_coupled,
                status.realtime_plane,
                status.control_plane
            );
            f.render_widget(
                Paragraph::new(s).block(Block::default().title("MoreData").borders(Borders::ALL)),
                chunks[0],
            );
            let a = format!(
                "backend={} host={} out={:?} sr={:?} ch={:?} pipewire={}",
                audio.backend,
                audio.host,
                audio.default_output,
                audio.sample_rate,
                audio.channels,
                audio.pipewire
            );
            f.render_widget(
                Paragraph::new(a).block(Block::default().title("audio").borders(Borders::ALL)),
                chunks[1],
            );
            let items: Vec<ListItem> = plugins
                .iter()
                .map(|p| {
                    ListItem::new(format!(
                        "{} [{}]",
                        p.name,
                        format!("{:?}", p.format).to_lowercase()
                    ))
                })
                .collect();
            f.render_widget(
                List::new(items)
                    .block(Block::default().title("native nodes").borders(Borders::ALL)),
                chunks[2],
            );
        })?;
        if event::poll(Duration::from_millis(200))?
            && let Event::Key(k) = event::read()?
            && k.code == KeyCode::Char('q')
        {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
