use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct MessageStore {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub id: String,
    pub peer_id: String,
    pub is_outgoing: bool,
    pub content: String,
    pub timestamp: u64,
    pub delivered: bool,
}

impl MessageStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                peer_id TEXT NOT NULL,
                is_outgoing INTEGER NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                delivered INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_messages_peer_id ON messages(peer_id);
            CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp);",
        )?;
        tracing::info!("opened message store at {}", path.display());
        Ok(Self { conn })
    }

    pub fn store_message(
        &self,
        id: &str,
        peer_id: &str,
        is_outgoing: bool,
        content: &str,
        timestamp: u64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO messages (id, peer_id, is_outgoing, content, timestamp, delivered)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, peer_id, is_outgoing as u8, content, timestamp, false],
        )?;
        Ok(())
    }

    pub fn get_conversation(&self, peer_id: &str, limit: u32) -> Result<Vec<StoredMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, peer_id, is_outgoing, content, timestamp, delivered
             FROM messages
             WHERE peer_id = ?1
             ORDER BY timestamp ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![peer_id, limit], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                peer_id: row.get(1)?,
                is_outgoing: row.get::<_, u8>(2)? != 0,
                content: row.get(3)?,
                timestamp: row.get(4)?,
                delivered: row.get::<_, u8>(5)? != 0,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    pub fn mark_delivered(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET delivered = 1 WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    pub fn all_conversations(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT peer_id FROM messages ORDER BY peer_id",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut peers = Vec::new();
        for row in rows {
            peers.push(row?);
        }
        Ok(peers)
    }
}
