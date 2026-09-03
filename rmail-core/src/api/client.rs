//! Minimal Microsoft Graph client used to pull mail, categories (labels)
//! and calendar events for an authenticated Outlook/365 account.
//!
//! Deliberately synchronous (`reqwest::blocking`) so callers - like the
//! TUI - can run requests on a plain background thread without pulling in
//! an async runtime.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::db::models::{CachedEmail, Label};

const GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";

/// Thin wrapper around a bearer-authenticated Microsoft Graph HTTP client.
pub struct GraphClient {
    http: reqwest::blocking::Client,
    access_token: String,
}

impl GraphClient {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            http: reqwest::blocking::Client::new(),
            access_token: access_token.into(),
        }
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, path_and_query: &str) -> Result<T> {
        let url = format!("{GRAPH_BASE_URL}{path_and_query}");
        let response = self
            .http
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .map_err(|e| anyhow!("request to {url} failed: {e}"))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(anyhow!("Graph API request to {url} failed ({status}): {body}"));
        }

        response
            .json::<T>()
            .map_err(|e| anyhow!("failed to parse response from {url}: {e}"))
    }

    /// Basic profile info for the signed-in account (used as the account
    /// identifier for token storage and display in the TUI).
    pub fn me(&self) -> Result<GraphUser> {
        self.get_json("/me?$select=displayName,mail,userPrincipalName")
    }

    /// Fetches the most recent inbox messages, newest first.
    pub fn list_messages(&self, top: u32) -> Result<Vec<GraphMessage>> {
        let query = format!(
            "/me/mailFolders/inbox/messages?$top={top}&$orderby=receivedDateTime desc\
             &$select=id,conversationId,subject,bodyPreview,isRead,receivedDateTime,from,categories"
        );
        let page: GraphPage<GraphMessage> = self.get_json(&query)?;
        Ok(page.value)
    }

    /// Fetches the user's Outlook categories, which this app maps onto
    /// Gmail-style labels.
    pub fn list_categories(&self) -> Result<Vec<Label>> {
        let page: GraphPage<GraphCategory> = self.get_json("/me/outlook/masterCategories")?;
        Ok(page.value.into_iter().map(GraphCategory::into_label).collect())
    }

    /// Fetches calendar events in `[now, now + days_ahead]`.
    pub fn list_events(&self, days_ahead: i64) -> Result<Vec<GraphEvent>> {
        let start = Utc::now();
        let end = start + chrono::Duration::days(days_ahead);
        let query = format!(
            "/me/calendarview?startDateTime={}&endDateTime={}&$orderby=start/dateTime\
             &$select=id,subject,start,end,location,isAllDay,organizer",
            start.to_rfc3339(),
            end.to_rfc3339(),
        );
        let page: GraphPage<GraphEvent> = self.get_json(&query)?;
        Ok(page.value)
    }
}

#[derive(Debug, Deserialize)]
struct GraphPage<T> {
    #[serde(rename = "value")]
    value: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphUser {
    pub display_name: Option<String>,
    pub mail: Option<String>,
    pub user_principal_name: Option<String>,
}

impl GraphUser {
    /// Best-effort email address: prefers `mail`, falls back to the UPN
    /// (some accounts, e.g. personal Microsoft accounts, have no `mail`).
    pub fn email(&self) -> Option<String> {
        self.mail.clone().or_else(|| self.user_principal_name.clone())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEmailAddress {
    pub name: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRecipient {
    pub email_address: Option<GraphEmailAddress>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphMessage {
    pub id: String,
    pub conversation_id: Option<String>,
    pub subject: Option<String>,
    pub body_preview: Option<String>,
    pub is_read: bool,
    pub received_date_time: String,
    pub from: Option<GraphRecipient>,
    #[serde(default)]
    pub categories: Vec<String>,
}

impl GraphMessage {
    /// Converts a Graph API message into the row shape stored locally in
    /// SQLite for offline/instant access.
    pub fn into_cached_email(self) -> CachedEmail {
        let received_at = DateTime::parse_from_rfc3339(&self.received_date_time)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let from_address = self
            .from
            .and_then(|r| r.email_address)
            .and_then(|a| a.address)
            .unwrap_or_default();

        CachedEmail {
            id: self.id,
            conversation_id: self.conversation_id.unwrap_or_default(),
            from_address,
            subject: self.subject.unwrap_or_default(),
            body_preview: self.body_preview.unwrap_or_default(),
            is_read: self.is_read,
            received_at,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphCategory {
    pub id: String,
    pub display_name: String,
    /// One of Outlook's preset color names (e.g. `"preset9"`), not a hex
    /// code.
    pub color: Option<String>,
}

impl GraphCategory {
    fn into_label(self) -> Label {
        Label {
            id: self.id,
            name: self.display_name,
            color: self.color,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDateTimeTimeZone {
    pub date_time: String,
    pub time_zone: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphLocation {
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEvent {
    pub id: String,
    pub subject: Option<String>,
    pub start: GraphDateTimeTimeZone,
    pub end: GraphDateTimeTimeZone,
    pub location: Option<GraphLocation>,
    pub is_all_day: bool,
    pub organizer: Option<GraphRecipient>,
}
