//! Rendering for the RMail TUI: a title bar, a row of tabs, per-tab
//! content, and a footer with status/keybinding hints - the same rough
//! shape as the GitHub Copilot CLI's terminal UI.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use crate::app::{App, Tab};

const ACCENT: Color = Color::Cyan;

pub fn render(frame: &mut Frame, app: &mut App) {
    let layout = Layout::vertical([
        Constraint::Length(1), // title
        Constraint::Length(1), // tabs
        Constraint::Fill(1),   // content
        Constraint::Length(1), // status/help
    ]);
    let [title_area, tabs_area, content_area, footer_area] = frame.area().layout(&layout);

    render_title(frame, title_area, app);
    render_tabs(frame, tabs_area, app);
    render_content(frame, content_area, app);
    render_footer(frame, footer_area, app);
}

fn render_title(frame: &mut Frame, area: Rect, app: &App) {
    let account = match &app.account {
        Some(email) => format!("Signed in as {email}"),
        None => "Not signed in".to_string(),
    };
    let line = Line::from_iter([
        Span::from(" 🦀 RMail ").bold().fg(ACCENT),
        Span::from("— Outlook client  ").dim(),
        Span::from(account).italic(),
    ]);
    frame.render_widget(line, area);
}

fn render_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles: Vec<&str> = Tab::ALL.iter().map(|t| t.title()).collect();
    let selected = Tab::ALL.iter().position(|t| *t == app.tab).unwrap_or(0);
    let tabs = Tabs::new(titles)
        .style(Color::Gray)
        .highlight_style(Style::default().fg(Color::Black).bg(ACCENT).bold())
        .select(selected)
        .divider(" ")
        .padding(" ", " ");
    frame.render_widget(tabs, area);
}

fn render_content(frame: &mut Frame, area: Rect, app: &mut App) {
    match app.tab {
        Tab::Inbox => render_inbox(frame, area, app),
        Tab::Labels => render_labels(frame, area, app),
        Tab::Calendar => render_calendar(frame, area, app),
        Tab::Account => render_account(frame, area, app),
    }
}

fn render_inbox(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.emails.is_empty() {
        render_placeholder(
            frame,
            area,
            "Inbox",
            "No cached messages yet. Sign in on the Account tab, then press 'r' to sync.",
        );
        return;
    }

    let items: Vec<ListItem> = app
        .emails
        .iter()
        .map(|email| {
            let unread_marker = if email.is_read { "  " } else { "● " };
            let subject = if email.subject.is_empty() {
                "(no subject)"
            } else {
                &email.subject
            };
            let top = Line::from_iter([
                Span::from(unread_marker).fg(ACCENT),
                Span::from(subject.to_string()).bold(),
            ]);
            let bottom = Line::from_iter([
                Span::from("    ".to_string()),
                Span::from(email.from_address.clone()).dim(),
                Span::from("  ".to_string()),
                Span::from(email.received_at.format("%Y-%m-%d %H:%M").to_string()).dim(),
            ]);
            ListItem::new(vec![top, bottom])
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::TOP).title(" Inbox "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.email_state);
}

fn render_labels(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.labels.is_empty() {
        render_placeholder(
            frame,
            area,
            "Labels",
            "No labels cached yet. Sign in on the Account tab, then press 'r' to sync \
             Outlook categories as labels.",
        );
        return;
    }

    let items: Vec<ListItem> = app
        .labels
        .iter()
        .map(|label| {
            let color = label.color.clone().unwrap_or_else(|| "none".to_string());
            ListItem::new(Line::from_iter([
                Span::from("🏷 ").fg(ACCENT),
                Span::from(label.name.clone()).bold(),
                Span::from(format!("  ({color})")).dim(),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::TOP).title(" Labels "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.label_state);
}

fn render_calendar(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.events.is_empty() {
        render_placeholder(
            frame,
            area,
            "Calendar",
            "No events loaded. Sign in on the Account tab, then press 'r' to fetch the next \
             14 days.",
        );
        return;
    }

    let items: Vec<ListItem> = app
        .events
        .iter()
        .map(|event| {
            let subject = event.subject.clone().unwrap_or_else(|| "(no title)".to_string());
            let location = event
                .location
                .as_ref()
                .and_then(|l| l.display_name.clone())
                .unwrap_or_default();
            let top = Line::from_iter([
                Span::from("📅 ").fg(ACCENT),
                Span::from(subject).bold(),
            ]);
            let bottom = Line::from_iter([
                Span::from("    ".to_string()),
                Span::from(event.start.date_time.clone()).dim(),
                Span::from("  ".to_string()),
                Span::from(location).dim(),
            ]);
            ListItem::new(vec![top, bottom])
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::TOP).title(" Calendar "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.event_state);
}

fn render_account(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![Line::from(Span::from("Account").bold())];
    match &app.account {
        Some(email) => lines.push(Line::from(format!("Signed in as: {email}"))),
        None => lines.push(Line::from("Not signed in.")),
    }
    lines.push(Line::from(""));
    match &app.config {
        Some(cfg) => {
            lines.push(Line::from(format!("Tenant: {}", cfg.tenant_id)));
            lines.push(Line::from(format!(
                "Redirect: http://localhost:{}/callback",
                cfg.redirect_port
            )));
        }
        None => {
            lines.push(Line::from(
                "RMAIL_CLIENT_ID is not set - login is disabled.".to_string(),
            ));
            lines.push(Line::from(
                "Register an Azure AD public client app and export RMAIL_CLIENT_ID \
                 (see README) to enable it."
                    .to_string(),
            ));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Press 'l' to sign in with Outlook.".dim()));
    lines.push(Line::from(
        "Press 'r' on Inbox/Labels/Calendar to sync from Microsoft Graph.".dim(),
    ));

    let block = Paragraph::new(lines)
        .block(Block::default().borders(Borders::TOP).title(" Account "))
        .wrap(Wrap { trim: false });
    frame.render_widget(block, area);
}

fn render_placeholder(frame: &mut Frame, area: Rect, title: &str, message: &str) {
    let block = Paragraph::new(message)
        .block(Block::default().borders(Borders::TOP).title(format!(" {title} ")))
        .wrap(Wrap { trim: true });
    frame.render_widget(block, area);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let hint = "Tab/Shift+Tab: switch  ↑/↓: select  r: sync  l: login  q: quit";
    let status = if app.busy {
        format!("⏳ {}", app.status)
    } else {
        app.status.clone()
    };
    let line = Line::from_iter([
        Span::from(format!(" {status}  ")).fg(ACCENT),
        Span::from(hint).dim(),
    ]);
    frame.render_widget(line, area);
}
