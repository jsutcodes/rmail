//! rmail-tui
//!
//! Terminal UI for RMail: sign in to Outlook/365 via OAuth, then browse
//! Inbox, Labels (Outlook categories), and Calendar from tabs across the
//! top - switch tabs with Tab/Shift+Tab, similar to the GitHub Copilot
//! CLI's TUI.

mod app;
mod ui;

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use app::App;

fn main() -> anyhow::Result<()> {
    let mut app = App::new()?;

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> anyhow::Result<()> {
    loop {
        app.poll_background();
        terminal.draw(|frame| ui::render(frame, app))?;

        // Short poll timeout keeps the UI responsive to background
        // login/sync results even without new key presses.
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(app, key.code);
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn handle_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Tab | KeyCode::Right => app.next_tab(),
        KeyCode::BackTab | KeyCode::Left => app.prev_tab(),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(),
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
        KeyCode::Char('l') => app.start_login(),
        KeyCode::Char('r') => app.start_sync(),
        _ => {}
    }
}
