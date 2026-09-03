//! High-level workflows that stitch together auth, the Graph API client,
//! and the local SQLite cache. This is the surface the desktop/mobile/TUI
//! shells call into - they shouldn't need to touch `api` or `db` directly.

use anyhow::Result;

use crate::api::auth::{self, AuthConfig, TokenSet};
use crate::api::client::{GraphClient, GraphEvent};
use crate::db::models::{CachedEmail, Label};
use crate::db::DbManager;

/// Runs the interactive OAuth login flow, persists the resulting tokens
/// in the OS keychain under the signed-in account's email address, and
/// returns that email address for use as the account id elsewhere.
pub fn login(config: &AuthConfig) -> Result<String> {
    let tokens = auth::authorize_interactive(config)?;
    let account = GraphClient::new(tokens.access_token.clone()).me()?;
    let account_id = account
        .email()
        .unwrap_or_else(|| "default".to_string());
    auth::save_tokens(&account_id, &tokens)?;
    Ok(account_id)
}

/// Returns a ready-to-use Graph client for `account`, refreshing the
/// stored access token first if needed.
pub fn graph_client_for(config: &AuthConfig, account: &str) -> Result<GraphClient> {
    let TokenSet { access_token, .. } = auth::ensure_valid_token(config, account)?;
    Ok(GraphClient::new(access_token))
}

/// Pulls the latest inbox messages from Microsoft Graph and upserts them
/// into the local cache. Returns the number of messages synced.
pub fn sync_inbox(db: &DbManager, client: &GraphClient, top: u32) -> Result<usize> {
    let messages = client.list_messages(top)?;
    let count = messages.len();
    for message in messages {
        db.upsert_email(&message.into_cached_email())?;
    }
    Ok(count)
}

/// Convenience read-through: cached emails, newest first.
pub fn cached_inbox(db: &DbManager) -> Result<Vec<CachedEmail>> {
    Ok(db.get_cached_emails()?)
}

/// Fetches the account's Outlook categories, mapped to labels, and
/// caches them locally for offline display.
pub fn sync_labels(db: &DbManager, client: &GraphClient) -> Result<Vec<Label>> {
    let labels = client.list_categories()?;
    for label in &labels {
        db.upsert_label(label)?;
    }
    Ok(labels)
}

/// Convenience read-through: cached labels, alphabetical.
pub fn cached_labels(db: &DbManager) -> Result<Vec<Label>> {
    Ok(db.get_cached_labels()?)
}

/// Fetches upcoming calendar events for the next `days_ahead` days.
pub fn upcoming_events(client: &GraphClient, days_ahead: i64) -> Result<Vec<GraphEvent>> {
    client.list_events(days_ahead)
}
