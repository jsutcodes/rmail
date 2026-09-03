use chrono::{DateTime, Utc};


// Represent Email Thread cached locally
#[derive(Debug, Clone)]
pub struct CachedEmail {
    pub id: String,
    pub conversation_id: String,
    pub from_address: String,
    pub subject: String,
    pub body_preview: String,
    pub is_read: bool,
    pub received_at: DateTime<Utc>,
}

// Represent Encrypted credentials 
#[derive(Debug, Clone)]
pub struct AccountCredentials{
    pub account_id: String,
    pub access_token: String,
    pub refresh_token: bool,
    pub expires_at: DateTime<Utc>,
}

// Represent Label 
#[derive(Debug, Clone)]
pub struct Label {
    pub id: String,         // Internal unique ID
    pub name: String,       // e.g., "Invoices", "Work", "Social"
    pub color: Option<String>, // Outlook categories support custom colors!
}

// Mapping cachedEmail and labels 
#[derive(Debug, Clone)]
pub struct EmailLabel {
    pub email_id: String,   // Foreign key to CachedEmail.id
    pub label_id: String,   // Foreign key to Label.id
}


