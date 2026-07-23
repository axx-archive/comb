//! Local metadata persistence for Comb.
//!
//! This store deliberately keeps identifiers, canonical digests, and fold
//! status only. Source message bodies remain in Buzz and are re-authorized on
//! access rather than copied into a shadow knowledge database.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

/// Persistence errors.
#[derive(Debug, Error)]
pub enum StoreError {
    /// SQLite operation failed.
    #[error("Comb metadata store failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// One durable artifact receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredArtifact {
    /// Stable Comb artifact identifier.
    pub stable_id: String,
    /// Proposal, review, record, or invalidation.
    pub artifact_kind: String,
    /// Buzz channel UUID as text.
    pub channel_id: String,
    /// Signed Buzz event ID, when published.
    pub event_id: Option<String>,
    /// Canonical digest of the Comb artifact.
    pub digest: String,
    /// Current folded status.
    pub status: String,
    /// Observation timestamp in Unix seconds.
    pub observed_at: i64,
}

/// One channel's durable replay cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChannelCursor {
    /// Buzz channel UUID as text.
    pub channel_id: String,
    /// Last fully processed source event ID.
    pub last_event_id: String,
    /// Cursor update timestamp in Unix seconds.
    pub updated_at: i64,
}

/// SQLite-backed metadata store.
pub struct StateStore {
    connection: Connection,
}

impl StateStore {
    /// Open or create a store on disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }

    /// Create an in-memory store for tests and disposable demos.
    pub fn in_memory() -> Result<Self, StoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, StoreError> {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS artifacts (
               stable_id TEXT PRIMARY KEY NOT NULL,
               artifact_kind TEXT NOT NULL,
               channel_id TEXT NOT NULL,
               event_id TEXT,
               digest TEXT NOT NULL,
               status TEXT NOT NULL,
               observed_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS artifacts_channel_idx
               ON artifacts(channel_id, artifact_kind, status);
             CREATE TABLE IF NOT EXISTS channel_cursors (
               channel_id TEXT PRIMARY KEY NOT NULL,
               last_event_id TEXT NOT NULL,
               updated_at INTEGER NOT NULL
             );",
        )?;
        Ok(Self { connection })
    }

    /// Insert or update an artifact receipt idempotently.
    pub fn put_artifact(&self, artifact: &StoredArtifact) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO artifacts
               (stable_id, artifact_kind, channel_id, event_id, digest, status, observed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(stable_id) DO UPDATE SET
               artifact_kind = excluded.artifact_kind,
               channel_id = excluded.channel_id,
               event_id = excluded.event_id,
               digest = excluded.digest,
               status = excluded.status,
               observed_at = excluded.observed_at",
            params![
                artifact.stable_id,
                artifact.artifact_kind,
                artifact.channel_id,
                artifact.event_id,
                artifact.digest,
                artifact.status,
                artifact.observed_at,
            ],
        )?;
        Ok(())
    }

    /// Read one artifact receipt by stable identifier.
    pub fn artifact(&self, stable_id: &str) -> Result<Option<StoredArtifact>, StoreError> {
        self.connection
            .query_row(
                "SELECT stable_id, artifact_kind, channel_id, event_id, digest, status, observed_at
                 FROM artifacts WHERE stable_id = ?1",
                [stable_id],
                |row| {
                    Ok(StoredArtifact {
                        stable_id: row.get(0)?,
                        artifact_kind: row.get(1)?,
                        channel_id: row.get(2)?,
                        event_id: row.get(3)?,
                        digest: row.get(4)?,
                        status: row.get(5)?,
                        observed_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Mark an existing artifact unsupported without changing its receipt.
    pub fn mark_unsupported(&self, stable_id: &str, observed_at: i64) -> Result<bool, StoreError> {
        let changed = self.connection.execute(
            "UPDATE artifacts SET status = 'unsupported', observed_at = ?2 WHERE stable_id = ?1",
            params![stable_id, observed_at],
        )?;
        Ok(changed == 1)
    }

    /// Persist a channel replay cursor.
    pub fn put_cursor(&self, cursor: &ChannelCursor) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO channel_cursors (channel_id, last_event_id, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(channel_id) DO UPDATE SET
               last_event_id = excluded.last_event_id,
               updated_at = excluded.updated_at",
            params![cursor.channel_id, cursor.last_event_id, cursor.updated_at],
        )?;
        Ok(())
    }

    /// Read a channel replay cursor.
    pub fn cursor(&self, channel_id: &str) -> Result<Option<ChannelCursor>, StoreError> {
        self.connection
            .query_row(
                "SELECT channel_id, last_event_id, updated_at
                 FROM channel_cursors WHERE channel_id = ?1",
                [channel_id],
                |row| {
                    Ok(ChannelCursor {
                        channel_id: row.get(0)?,
                        last_event_id: row.get(1)?,
                        updated_at: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(status: &str, observed_at: i64) -> StoredArtifact {
        StoredArtifact {
            stable_id: "knowledge.launch-date.v1".into(),
            artifact_kind: "record".into(),
            channel_id: "3eb4d915-fcbf-4d7a-9fc6-7bc70a1e7c3e".into(),
            event_id: Some("a".repeat(64)),
            digest: "b".repeat(64),
            status: status.into(),
            observed_at,
        }
    }

    #[test]
    fn repeated_receipt_is_idempotent_and_updates_status() -> Result<(), StoreError> {
        let store = StateStore::in_memory()?;
        store.put_artifact(&artifact("ratified", 10))?;
        store.put_artifact(&artifact("ratified", 10))?;
        assert_eq!(
            store.artifact("knowledge.launch-date.v1")?,
            Some(artifact("ratified", 10))
        );
        assert!(store.mark_unsupported("knowledge.launch-date.v1", 20)?);
        let loaded = store
            .artifact("knowledge.launch-date.v1")?
            .expect("artifact exists");
        assert_eq!(loaded.status, "unsupported");
        assert_eq!(loaded.observed_at, 20);
        Ok(())
    }

    #[test]
    fn cursor_survives_reopen() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("comb.db");
        {
            let store = StateStore::open(&path)?;
            store.put_cursor(&ChannelCursor {
                channel_id: "channel-a".into(),
                last_event_id: "c".repeat(64),
                updated_at: 42,
            })?;
        }
        let reopened = StateStore::open(path)?;
        assert_eq!(
            reopened.cursor("channel-a")?,
            Some(ChannelCursor {
                channel_id: "channel-a".into(),
                last_event_id: "c".repeat(64),
                updated_at: 42,
            })
        );
        Ok(())
    }

    #[test]
    fn schema_has_no_source_content_column() -> Result<(), StoreError> {
        let store = StateStore::in_memory()?;
        let mut statement = store.connection.prepare("PRAGMA table_info(artifacts)")?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        assert!(!names
            .iter()
            .any(|name| name.contains("content") || name.contains("body")));
        Ok(())
    }
}
