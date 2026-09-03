//! Application state for the RMail TUI: which tab is active, the cached
//! data shown in each tab, and the background-thread plumbing used for
//! login/sync so the UI never blocks on network I/O.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use ratatui::widgets::ListState;
use rmail_core::api::auth::AuthConfig;
use rmail_core::api::client::GraphEvent;
use rmail_core::db::models::{CachedEmail, Label};
use rmail_core::db::DbManager;
use rmail_core::services;

/// Top-level tabs, switched with Tab/Shift+Tab or the Left/Right arrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Inbox,
    Labels,
    Calendar,
    Account,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Inbox, Tab::Labels, Tab::Calendar, Tab::Account];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Inbox => "Inbox",
            Tab::Labels => "Labels",
            Tab::Calendar => "Calendar",
            Tab::Account => "Account",
        }
    }

    fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    fn from_index(index: usize) -> Tab {
        Tab::ALL[index % Tab::ALL.len()]
    }
}

/// Results of background operations, delivered back to the UI thread.
pub enum BgMsg {
    LoginDone(Result<String, String>),
    InboxSynced(Result<usize, String>),
    LabelsSynced(Result<Vec<Label>, String>),
    EventsFetched(Result<Vec<GraphEvent>, String>),
}

pub struct App {
    pub tab: Tab,
    pub should_quit: bool,
    pub busy: bool,
    pub status: String,

    pub db: DbManager,
    pub config: Option<AuthConfig>,
    pub account: Option<String>,

    pub emails: Vec<CachedEmail>,
    pub email_state: ListState,
    pub labels: Vec<Label>,
    pub label_state: ListState,
    pub events: Vec<GraphEvent>,
    pub event_state: ListState,

    bg_tx: Sender<BgMsg>,
    bg_rx: Receiver<BgMsg>,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let data_dir = data_dir();
        std::fs::create_dir_all(&data_dir)?;
        let db = DbManager::new(data_dir.join("cache.db"))?;

        let config = match AuthConfig::from_env() {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                // Still usable in offline/read-cache mode; surfaced on the
                // Account tab so the user knows how to fix it.
                eprintln!("note: {e}");
                None
            }
        };

        let account = load_last_account(&data_dir);
        let emails = services::cached_inbox(&db).unwrap_or_default();
        let labels = services::cached_labels(&db).unwrap_or_default();

        let (bg_tx, bg_rx) = mpsc::channel();

        let mut email_state = ListState::default();
        if !emails.is_empty() {
            email_state.select(Some(0));
        }
        let mut label_state = ListState::default();
        if !labels.is_empty() {
            label_state.select(Some(0));
        }

        let status = if account.is_some() {
            "Ready. Press 'r' to sync, 'q' to quit.".to_string()
        } else if config.is_some() {
            "Not signed in. Press 'l' on the Account tab to log in with Outlook.".to_string()
        } else {
            "Set RMAIL_CLIENT_ID to enable Outlook login (see README). Showing cached data only."
                .to_string()
        };

        Ok(Self {
            tab: Tab::Inbox,
            should_quit: false,
            busy: false,
            status,
            db,
            config,
            account,
            emails,
            email_state,
            labels,
            label_state,
            events: Vec::new(),
            event_state: ListState::default(),
            bg_tx,
            bg_rx,
        })
    }

    pub fn next_tab(&mut self) {
        self.tab = Tab::from_index(self.tab.index() + 1);
    }

    pub fn prev_tab(&mut self) {
        self.tab = Tab::from_index(self.tab.index() + Tab::ALL.len() - 1);
    }

    pub fn select_next(&mut self) {
        match self.tab {
            Tab::Inbox => self.email_state.select_next(),
            Tab::Labels => self.label_state.select_next(),
            Tab::Calendar => self.event_state.select_next(),
            Tab::Account => {}
        }
    }

    pub fn select_prev(&mut self) {
        match self.tab {
            Tab::Inbox => self.email_state.select_previous(),
            Tab::Labels => self.label_state.select_previous(),
            Tab::Calendar => self.event_state.select_previous(),
            Tab::Account => {}
        }
    }

    /// Kicks off the interactive OAuth login flow on a background thread.
    pub fn start_login(&mut self) {
        let Some(config) = self.config.clone() else {
            self.status = "Cannot log in: RMAIL_CLIENT_ID is not set.".to_string();
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = "Opening browser to sign in to Outlook...".to_string();
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let result = services::login(&config).map_err(|e| e.to_string());
            let _ = tx.send(BgMsg::LoginDone(result));
        });
    }

    /// Refreshes the data for the active tab from Microsoft Graph.
    pub fn start_sync(&mut self) {
        let Some(config) = self.config.clone() else {
            self.status = "Cannot sync: RMAIL_CLIENT_ID is not set.".to_string();
            return;
        };
        let Some(account) = self.account.clone() else {
            self.status = "Sign in first (press 'l' on the Account tab).".to_string();
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = "Syncing...".to_string();

        let tx = self.bg_tx.clone();
        let db = self.db.clone();
        match self.tab {
            Tab::Inbox => {
                thread::spawn(move || {
                    let result = services::graph_client_for(&config, &account)
                        .and_then(|client| services::sync_inbox(&db, &client, 50))
                        .map_err(|e| e.to_string());
                    let _ = tx.send(BgMsg::InboxSynced(result));
                });
            }
            Tab::Labels => {
                thread::spawn(move || {
                    let result = services::graph_client_for(&config, &account)
                        .and_then(|client| services::sync_labels(&db, &client))
                        .map_err(|e| e.to_string());
                    let _ = tx.send(BgMsg::LabelsSynced(result));
                });
            }
            Tab::Calendar => {
                thread::spawn(move || {
                    let result = services::graph_client_for(&config, &account)
                        .and_then(|client| services::upcoming_events(&client, 14))
                        .map_err(|e| e.to_string());
                    let _ = tx.send(BgMsg::EventsFetched(result));
                });
            }
            Tab::Account => {
                self.busy = false;
                self.status = "Nothing to sync here; try Inbox, Labels, or Calendar.".to_string();
            }
        }
    }

    /// Drains any completed background operations and applies their
    /// results to the UI state. Called once per event-loop tick.
    pub fn poll_background(&mut self) {
        while let Ok(msg) = self.bg_rx.try_recv() {
            self.busy = false;
            match msg {
                BgMsg::LoginDone(Ok(account)) => {
                    save_last_account(&data_dir(), &account);
                    self.account = Some(account.clone());
                    self.status = format!("Signed in as {account}.");
                }
                BgMsg::LoginDone(Err(e)) => {
                    self.status = format!("Login failed: {e}");
                }
                BgMsg::InboxSynced(Ok(count)) => {
                    self.emails = services::cached_inbox(&self.db).unwrap_or_default();
                    if self.email_state.selected().is_none() && !self.emails.is_empty() {
                        self.email_state.select(Some(0));
                    }
                    self.status = format!("Synced {count} messages.");
                }
                BgMsg::InboxSynced(Err(e)) => self.status = format!("Inbox sync failed: {e}"),
                BgMsg::LabelsSynced(Ok(labels)) => {
                    self.labels = labels;
                    if self.label_state.selected().is_none() && !self.labels.is_empty() {
                        self.label_state.select(Some(0));
                    }
                    self.status = format!("Synced {} labels.", self.labels.len());
                }
                BgMsg::LabelsSynced(Err(e)) => self.status = format!("Label sync failed: {e}"),
                BgMsg::EventsFetched(Ok(events)) => {
                    self.events = events;
                    if self.event_state.selected().is_none() && !self.events.is_empty() {
                        self.event_state.select(Some(0));
                    }
                    self.status = format!("Fetched {} upcoming events.", self.events.len());
                }
                BgMsg::EventsFetched(Err(e)) => self.status = format!("Calendar fetch failed: {e}"),
            }
        }
    }
}

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("rmail")
}

fn account_file(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("account.txt")
}

fn load_last_account(data_dir: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(account_file(data_dir))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn save_last_account(data_dir: &std::path::Path, account: &str) {
    let _ = std::fs::write(account_file(data_dir), account);
}
