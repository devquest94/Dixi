//! The local history of the text changes Dixi made.
//!
//! Whenever a transformation, a snippet or a cleanup replaces something the
//! wrote, the original text and the result are stored here. That is what
//! makes the change reversible: the user can put back what was there before.
//!
//! What is stored is **only** the text Dixi itself produced or replaced, and
//! the name of the application. The history never reads the content of the
//! application on its own, and it is never sent anywhere.
//!
//! # Retention
//!
//! The history has a retention period. When it expires, the entries are
//! deleted locally. The user can also clear it at any time.

use std::path::PathBuf;
use std::sync::RwLock;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use super::personal_store::PersonalStore;

/// The default retention of the history, in days.
pub const DEFAULT_RETENTION_DAYS: i64 = 30;

/// The kind of change that was recorded.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HistoryKind {
    /// A transform of an existing text.
    Transform,
    /// The expansion of a snippet.
    Snippet,
    /// A cleanup of the transcription.
    Cleanup,
}

impl HistoryKind {
    /// The wire value.
    pub fn as_str(&self) -> &'static str {
        match self {
            HistoryKind::Transform => "transform",
            HistoryKind::Snippet => "snippet",
            HistoryKind::Cleanup => "cleanup",
        }
    }

    /// Parses a stored value.
    pub fn parse(text: &str) -> Self {
        match text.trim().to_lowercase().as_str() {
            "transform" => HistoryKind::Transform,
            "snippet" => HistoryKind::Snippet,
            _ => HistoryKind::Cleanup,
        }
    }
}

/// One recorded change.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct TextHistoryEntry {
    pub id: i64,
    pub created_at: i64,
    /// The application the change was made in, when it was known.
    pub app_name: Option<String>,
    pub kind: HistoryKind,
    /// The transform or snippet that caused the change.
    pub source_id: Option<String>,
    /// The text before the change.
    pub original_text: String,
    /// The text after the change.
    pub result_text: String,
    /// Whether the change was already reverted.
    pub undone: bool,
}

/// Owns the local history of text changes.
pub struct TextHistoryManager {
    store: PersonalStore,
    entries: RwLock<Vec<TextHistoryEntry>>,
    retention_days: RwLock<i64>,
}

impl TextHistoryManager {
    /// Opens the personal database and loads the history.
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        Self::with_store(PersonalStore::new(app_handle)?)
    }

    /// Opens the personal database at an explicit path.
    pub fn with_db_path(db_path: PathBuf) -> Result<Self, String> {
        Self::with_store(PersonalStore::with_db_path(db_path)?)
    }

    /// Uses an already opened personal database.
    pub fn with_store(store: PersonalStore) -> Result<Self, String> {
        let manager = Self {
            store,
            entries: RwLock::new(Vec::new()),
            retention_days: RwLock::new(DEFAULT_RETENTION_DAYS),
        };
        manager.reload()?;
        Ok(manager)
    }

    fn connection(&self) -> Result<Connection, String> {
        self.store.connection()
    }

    /// Re-reads the history from disk, newest first.
    fn reload(&self) -> Result<(), String> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, created_at, app_name, kind, source_id, original_text, result_text, undone
                 FROM text_history
                 ORDER BY id DESC",
            )
            .map_err(|e| format!("Failed to prepare the text history query: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(TextHistoryEntry {
                    id: row.get("id")?,
                    created_at: row.get("created_at")?,
                    app_name: row.get("app_name")?,
                    kind: HistoryKind::parse(&row.get::<_, String>("kind")?),
                    source_id: row.get("source_id")?,
                    original_text: row.get("original_text")?,
                    result_text: row.get("result_text")?,
                    undone: row.get::<_, i64>("undone")? != 0,
                })
            })
            .map_err(|e| format!("Failed to read the text history: {e}"))?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row.map_err(|e| format!("Failed to read a history entry: {e}"))?);
        }
        *self
            .entries
            .write()
            .map_err(|_| "Text history lock poisoned".to_string())? = entries;
        Ok(())
    }

    /// Records a change.
    ///
    /// A change that did not modify anything is not recorded: the history is
    /// for undoing real modifications.
    pub fn record(
        &self,
        kind: HistoryKind,
        source_id: Option<&str>,
        app_name: Option<&str>,
        original_text: &str,
        result_text: &str,
    ) -> Result<Option<i64>, String> {
        if original_text == result_text {
            return Ok(None);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO text_history
                (created_at, app_name, kind, source_id, original_text, result_text, undone)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
            params![
                now,
                app_name,
                kind.as_str(),
                source_id,
                original_text,
                result_text,
            ],
        )
        .map_err(|e| format!("Failed to record the text history: {e}"))?;

        let id = conn.last_insert_rowid();
        self.reload()?;
        Ok(Some(id))
    }

    /// The history, newest first.
    pub fn list(&self, limit: usize) -> Vec<TextHistoryEntry> {
        self.entries
            .read()
            .map(|entries| entries.iter().take(limit).cloned().collect())
            .unwrap_or_default()
    }

    /// One entry.
    pub fn get(&self, id: i64) -> Option<TextHistoryEntry> {
        self.entries
            .read()
            .ok()
            .and_then(|entries| entries.iter().find(|entry| entry.id == id).cloned())
    }

    /// The most recent change that can still be reverted.
    pub fn last_undoable(&self) -> Option<TextHistoryEntry> {
        self.entries
            .read()
            .ok()
            .and_then(|entries| entries.iter().find(|entry| !entry.undone).cloned())
    }

    /// Marks a change as reverted.
    ///
    /// The entry is kept so the user can still see what happened; it is just
    /// not offered as undoable again.
    pub fn mark_undone(&self, id: i64) -> Result<(), String> {
        let conn = self.connection()?;
        let changed = conn
            .execute(
                "UPDATE text_history SET undone = 1 WHERE id = ?1",
                params![id],
            )
            .map_err(|e| format!("Failed to mark the history entry as reverted: {e}"))?;
        if changed == 0 {
            return Err(format!("History entry {id} not found"));
        }
        self.reload()?;
        Ok(())
    }

    /// Removes every entry.
    pub fn clear(&self) -> Result<(), String> {
        let conn = self.connection()?;
        conn.execute("DELETE FROM text_history", [])
            .map_err(|e| format!("Failed to clear the text history: {e}"))?;
        self.reload()?;
        Ok(())
    }

    /// How long the entries are kept, in days.
    pub fn retention_days(&self) -> i64 {
        self.retention_days
            .read()
            .map(|days| *days)
            .unwrap_or(DEFAULT_RETENTION_DAYS)
    }

    /// Sets how long the entries are kept.
    ///
    /// A retention of zero means the history is kept until the user clears
    /// it. A negative retention is refused.
    pub fn set_retention_days(&self, days: i64) -> Result<(), String> {
        if days < 0 {
            return Err("The retention cannot be negative".to_string());
        }
        *self
            .retention_days
            .write()
            .map_err(|_| "Text history lock poisoned".to_string())? = days;
        Ok(())
    }

    /// Removes the entries that are older than the retention.
    ///
    /// Returns how many were removed.
    pub fn purge_expired(&self) -> Result<usize, String> {
        let days = self.retention_days();
        if days == 0 {
            return Ok(0);
        }

        let cutoff = chrono::Utc::now().timestamp_millis() - days * 24 * 60 * 60 * 1000;
        let conn = self.connection()?;
        let removed = conn
            .execute("DELETE FROM text_history WHERE created_at < ?1", params![cutoff])
            .map_err(|e| format!("Failed to purge the text history: {e}"))?;
        if removed > 0 {
            self.reload()?;
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn manager() -> (TempDir, TextHistoryManager) {
        let dir = TempDir::new().expect("a temporary folder");
        let manager =
            TextHistoryManager::with_db_path(dir.path().join("dixi.db")).expect("the manager opens");
        (dir, manager)
    }

    #[test]
    fn a_change_is_recorded_and_read_back() {
        let (_dir, manager) = manager();
        let id = manager
            .record(
                HistoryKind::Transform,
                Some("formalizar"),
                Some("notepad.exe"),
                "hola",
                "buenos dias",
            )
            .expect("recorded")
            .expect("an id");

        let entry = manager.get(id).expect("the entry exists");
        assert_eq!(entry.original_text, "hola");
        assert_eq!(entry.result_text, "buenos dias");
        assert_eq!(entry.kind, HistoryKind::Transform);
        assert_eq!(entry.source_id.as_deref(), Some("formalizar"));
        assert_eq!(entry.app_name.as_deref(), Some("notepad.exe"));
        assert!(!entry.undone);
    }

    #[test]
    fn a_change_that_changes_nothing_is_not_recorded() {
        let (_dir, manager) = manager();
        let id = manager
            .record(HistoryKind::Cleanup, None, None, "igual", "igual")
            .expect("recorded");
        assert_eq!(id, None);
        assert!(manager.list(10).is_empty());
    }

    #[test]
    fn the_history_is_newest_first() {
        let (_dir, manager) = manager();
        manager
            .record(HistoryKind::Transform, None, None, "a", "b")
            .expect("recorded");
        manager
            .record(HistoryKind::Transform, None, None, "c", "d")
            .expect("recorded");

        let entries = manager.list(10);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].original_text, "c");
    }

    #[test]
    fn the_limit_cuts_the_history() {
        let (_dir, manager) = manager();
        for index in 0..5 {
            manager
                .record(HistoryKind::Transform, None, None, "a", &format!("b{index}"))
                .expect("recorded");
        }
        assert_eq!(manager.list(2).len(), 2);
    }

    #[test]
    fn a_reverted_change_is_not_offered_again() {
        let (_dir, manager) = manager();
        let id = manager
            .record(HistoryKind::Transform, None, None, "a", "b")
            .expect("recorded")
            .expect("an id");

        assert_eq!(manager.last_undoable().map(|entry| entry.id), Some(id));
        manager.mark_undone(id).expect("marked");

        assert!(manager.last_undoable().is_none());
        assert!(manager.get(id).expect("the entry is still there").undone);
    }

    #[test]
    fn the_history_can_be_cleared() {
        let (_dir, manager) = manager();
        manager
            .record(HistoryKind::Transform, None, None, "a", "b")
            .expect("recorded");
        manager.clear().expect("cleared");
        assert!(manager.list(10).is_empty());
    }

    #[test]
    fn the_expired_entries_are_removed() {
        let (_dir, manager) = manager();
        manager
            .record(HistoryKind::Transform, None, None, "a", "b")
            .expect("recorded");

        // With a retention of zero nothing expires.
        manager.set_retention_days(0).expect("set");
        assert_eq!(manager.purge_expired().expect("purged"), 0);
        assert_eq!(manager.list(10).len(), 1);

        // With a retention of one day the fresh entry stays.
        manager.set_retention_days(1).expect("set");
        assert_eq!(manager.purge_expired().expect("purged"), 0);
        assert_eq!(manager.list(10).len(), 1);
    }

    #[test]
    fn a_negative_retention_is_refused() {
        let (_dir, manager) = manager();
        assert!(manager.set_retention_days(-1).is_err());
    }

    #[test]
    fn the_history_survives_a_restart() {
        let dir = TempDir::new().expect("a temporary folder");
        let path = dir.path().join("dixi.db");
        {
            let manager = TextHistoryManager::with_db_path(path.clone()).expect("opened");
            manager
                .record(HistoryKind::Snippet, Some("firma"), None, "mi firma", "Juan Perez")
                .expect("recorded");
        }
        let reopened = TextHistoryManager::with_db_path(path).expect("reopened");
        let entries = reopened.list(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, HistoryKind::Snippet);
        assert_eq!(entries[0].result_text, "Juan Perez");
    }
}
