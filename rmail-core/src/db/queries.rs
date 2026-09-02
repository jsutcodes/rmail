pub const ENABLE_WAL_SQL: &str = "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON";

pub const UPSERT_EMAIL_SQL: &str = "
    INSERT INTO emails (id, conversation_id, from_address, subject, body_preview, is_read, received_at)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
    ON CONFLICT(id) DO UPDATE SET
    is_read = excluded.is_read,
    body_preview = excluded.body_preview
";


pub const FETCH_INBOX_SQL: &str = "
    SELECT id, conversation_id, from_address, subject, body_preview, is_read, received_at
    FROM emails
    ORDER BY received_at DESC
";
