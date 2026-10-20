//! The personal database.
//!
//! One SQLite file holds everything the user writes: the dictionary, the
//! writing styles and profiles, the snippets, the transforms and the local
//! history of text changes.
//!
//! Sharing one database is not a convenience: the writing profiles change how
//! the dictionary and the snippets are applied, so they are facets of the same
//! object, not separate systems. A second database would mean a second set of
//! migrations, a second backup and a second thing to keep in sync.
//!
//! The migrations are versioned with `rusqlite_migration`, the same mechanism
//! the history database uses, so a store written by an older version is
//! brought forward in place.

use std::path::PathBuf;

use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use tauri::AppHandle;

use crate::portable::app_data_dir;

/// The name of the database file inside the application data directory.
pub const DATABASE_FILE: &str = "dixi.db";

/// Every migration, in order.
static MIGRATIONS: &[M] = &[
    // --- The dictionary -------------------------------------------------
    M::up(
        "CREATE TABLE IF NOT EXISTS dictionary_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            term TEXT NOT NULL,
            replacement TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT 'term',
            enabled INTEGER NOT NULL DEFAULT 1,
            scope TEXT NOT NULL DEFAULT 'global',
            case_sensitive INTEGER NOT NULL DEFAULT 0,
            whole_word INTEGER NOT NULL DEFAULT 1,
            hit_count INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );",
    ),
    // The replacement is part of the key on purpose: two rules may share a
    // term and differ in what they write, and the priority is what decides
    // between them. Only an exact repetition is refused.
    M::up(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_dictionary_unique
            ON dictionary_entries (term COLLATE NOCASE, scope, kind, replacement);",
    ),
    M::up("ALTER TABLE dictionary_entries ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;"),
    M::up("ALTER TABLE dictionary_entries ADD COLUMN language TEXT;"),
    // --- Writing styles -------------------------------------------------
    M::up(
        "CREATE TABLE IF NOT EXISTS writing_styles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'builtin',
            base_prompt TEXT NOT NULL DEFAULT '',
            cleanup_level TEXT NOT NULL DEFAULT 'light',
            ai_cleanup INTEGER NOT NULL DEFAULT 0,
            format_rules TEXT NOT NULL DEFAULT '[]',
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );",
    ),
    // --- Writing profiles ------------------------------------------------
    M::up(
        "CREATE TABLE IF NOT EXISTS writing_profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'builtin',
            app_names TEXT NOT NULL DEFAULT '[]',
            language TEXT,
            style_id TEXT NOT NULL,
            cleanup_level TEXT,
            ai_cleanup INTEGER,
            format_rules TEXT NOT NULL DEFAULT '[]',
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );",
    ),
    M::up(
        "CREATE TABLE IF NOT EXISTS profile_overrides (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            scope_kind TEXT NOT NULL,
            scope_value TEXT NOT NULL,
            style_id TEXT,
            cleanup_level TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );",
    ),
    // --- Snippets --------------------------------------------------------
    M::up(
        "CREATE TABLE IF NOT EXISTS snippets (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            trigger_phrase TEXT NOT NULL,
            expansion TEXT NOT NULL,
            variables TEXT NOT NULL DEFAULT '[]',
            category TEXT,
            app_names TEXT NOT NULL DEFAULT '[]',
            language TEXT,
            is_favorite INTEGER NOT NULL DEFAULT 0,
            enabled INTEGER NOT NULL DEFAULT 1,
            usage_count INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );",
    ),
    // --- Transforms ------------------------------------------------------
    M::up(
        "CREATE TABLE IF NOT EXISTS transforms (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'builtin',
            prompt TEXT NOT NULL,
            provider_id TEXT,
            model TEXT,
            shortcut TEXT,
            category TEXT,
            enabled INTEGER NOT NULL DEFAULT 1,
            usage_count INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );",
    ),
    // --- Local history of text changes ------------------------------------
    M::up(
        "CREATE TABLE IF NOT EXISTS text_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at INTEGER NOT NULL,
            app_name TEXT,
            kind TEXT NOT NULL,
            source_id TEXT,
            original_text TEXT NOT NULL,
            result_text TEXT NOT NULL
        );",
    ),
    // An entry that was reverted is kept (the user may want to see what
    // happened) but is not offered as undoable again.
    M::up("ALTER TABLE text_history ADD COLUMN undone INTEGER NOT NULL DEFAULT 0;"),
];

/// Owns the personal database file and the migrations.
pub struct PersonalStore {
    db_path: PathBuf,
}

impl PersonalStore {
    /// Opens (or creates) the database in the application data directory.
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let app_data_dir = app_data_dir(app_handle)
            .map_err(|e| format!("Failed to resolve the application data directory: {e}"))?;
        Self::with_db_path(app_data_dir.join(DATABASE_FILE))
    }

    /// Opens (or creates) the database at an explicit path.
    ///
    /// The path-based constructor is the seam that keeps the store
    /// independent of the Tauri application handle, and it is what lets the
    /// tests work on an isolated database.
    pub fn with_db_path(db_path: PathBuf) -> Result<Self, String> {
        let store = Self { db_path };
        store.migrate()?;
        Ok(store)
    }

    /// The path of the database file.
    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }

    /// Opens a connection to the database.
    pub fn connection(&self) -> Result<Connection, String> {
        Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open the personal database: {e}"))
    }

    /// Applies the pending migrations.
    pub fn migrate(&self) -> Result<(), String> {
        let mut conn = self.connection()?;
        let migrations = Migrations::new(MIGRATIONS.to_vec());

        #[cfg(debug_assertions)]
        migrations.validate().expect("Invalid migrations");

        migrations
            .to_latest(&mut conn)
            .map_err(|e| format!("Failed to apply the personal database migrations: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn a_fresh_database_has_every_table() {
        let dir = TempDir::new().expect("a temporary folder");
        let store =
            PersonalStore::with_db_path(dir.path().join(DATABASE_FILE)).expect("the store opens");
        let conn = store.connection().expect("a connection");

        for table in [
            "dictionary_entries",
            "writing_styles",
            "writing_profiles",
            "profile_overrides",
            "snippets",
            "transforms",
            "text_history",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap_or_else(|e| panic!("the table {table} must exist: {e}"));
            assert_eq!(count, 1, "the table {table} must exist");
        }
    }

    #[test]
    fn the_migrations_are_idempotent() {
        let dir = TempDir::new().expect("a temporary folder");
        let path = dir.path().join(DATABASE_FILE);
        PersonalStore::with_db_path(path.clone()).expect("the store opens");
        // Reopening applies the migrations again, and must not fail.
        PersonalStore::with_db_path(path).expect("reopening must be safe");
    }

    #[test]
    fn the_dictionary_table_keeps_its_columns() {
        let dir = TempDir::new().expect("a temporary folder");
        let store =
            PersonalStore::with_db_path(dir.path().join(DATABASE_FILE)).expect("the store opens");
        let conn = store.connection().expect("a connection");
        let mut stmt = conn
            .prepare("PRAGMA table_info(dictionary_entries)")
            .expect("the table exists");
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>("name"))
            .expect("the columns are readable")
            .filter_map(|c| c.ok())
            .collect();
        for column in ["term", "replacement", "kind", "scope", "priority", "language"] {
            assert!(
                columns.iter().any(|c| c == column),
                "the column {column} must exist, got: {columns:?}"
            );
        }
    }
}
