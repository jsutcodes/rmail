pub mod models;
pub mod schema;
pub mod queries;

use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};

// Thread safe db connection wrapper shared between desktop/mobile
#[derive(Clone)]
pub struct DbManager {
    conn: Arc<Mutex<Connection>>,
}

impl DbManager {
    // Opens or creates the local SQLite db file (at ~/.config/outlook/cache.db)
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        //Enables Write ahead logging (WAL) for faster perfomance and concurrency
        conn.execute_batch(queries::ENABLE_WAL)?;
        conn.execute_batch(schema::CREATE_TABLES_SQL)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
        
    }

    // Insert or Update
    pub fn upsert_email(&self, email: &models::LocalEmail) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            queries::UPSERT_EMAIL,
            params![
                email.id,
                email.conversation_id,
                email.from_address,
                email.subject,
                email.body_preview,
                if email.is_read { 1 } else { 0 },
                email.received_at.to_rfc3339()
            ],
          )?;
            Ok(())
    }

    // Fetch email for the primary thrad list (Gmail-style view)
    pub fn get_cached_emails(&self) -> Result<Vec<models::CachedEmail>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(queries::FETCH_INBOX);

        let email_itr = stmt.query_map([], |row| {
            let date_str: String = row.get(6)?;
            let recieved_at = chrono::DateTime::parse_from_rfc3339(&date_str)
                .unwrap_or_default()
                .with_timezone(&chrono::Utc);

            Ok(models::CachedEmail {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                from_address: row.get(2)?,
                subject: row.get(3)?,
                body_preview: row.get(4)?,
                is_read: row.get<<_, i32>>(5)? == 1,
                received_at,
            })
        })?;
        
        let mut emails = Vec::new();
        for email in email_itr {
            email.push(email?);
        }
        Ok(emails)
    }
}
