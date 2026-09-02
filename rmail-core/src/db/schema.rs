
pub const CREATE_TABLES_SQL: &str = "
CREATE TABLE IF NOT EXISTS credentials (
    account_id TEXT PRIMARY KEY,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    expires_at TEXT NOT NULL
    );

CREATE TABLE IF NOT EXISTS emails (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    from_address TEXT NOT NULL,
    subject TEXT NOT NULL,
    body_preview TEXT,
    is_read INTEGER NOT NULL DEFAULT 0,
    received_at TEXT NOT NULL
    );

CREATE INDEX IF NOT EXISTS idx_email_coversation ON emails(conversation_id);

";
