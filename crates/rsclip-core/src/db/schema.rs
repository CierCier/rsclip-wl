use anyhow::Result;
use rusqlite::params;

use super::Database;

/// Highest applied schema version. Add a new `if current < N` step when changing schema.
const SCHEMA_USER_VERSION: i32 = 3;

impl Database {
    pub fn migrate(&self) -> Result<()> {
        let current: i32 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current >= SCHEMA_USER_VERSION {
            return Ok(());
        }

        // Apply pending steps in order. New schema work becomes `if current < 2 { ... }`, etc.
        if current < 1 {
            self.migrate_v1()?;
        }
        if current < 2 {
            self.migrate_v2()?;
        }
        if current < 3 {
            self.migrate_v3()?;
        }

        self.conn
            .pragma_update(None, "user_version", SCHEMA_USER_VERSION)?;
        Ok(())
    }

    /// Performance covering indexes for all SortMode variants (Type, MostUsed) and kind filtering.
    fn migrate_v3(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_entries_kind_pinned_updated ON entries(deleted, kind, pinned DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_type_sort ON entries(deleted, kind ASC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_most_used ON entries(deleted, use_count DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_kind_used ON entries(deleted, kind, use_count DESC, updated_at DESC);
            "#,
        )?;
        Ok(())
    }

    /// Performance indexes for recent and pinned sorting without table scans.
    fn migrate_v2(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_entries_updated_at ON entries(deleted, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_pinned_updated ON entries(deleted, pinned DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_secrets_updated_at ON secrets(deleted, updated_at DESC);
            "#,
        )?;
        Ok(())
    }

    /// Baseline schema: tables, columns, indexes, and one-time link repair.
    fn migrate_v1(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS entries (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              content_hash TEXT NOT NULL,
              kind TEXT NOT NULL,
              mime_type TEXT NOT NULL,
              title TEXT NOT NULL,
              preview_text TEXT,
              text_content TEXT,
              file_path TEXT,
              thumb_path TEXT,
              source_app TEXT,
              link_url TEXT,
              link_domain TEXT,
              link_icon TEXT,
              color_value TEXT,
              color_format TEXT,
              pinned INTEGER NOT NULL DEFAULT 0,
              copied_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              last_used_at INTEGER,
              use_count INTEGER NOT NULL DEFAULT 0,
              size_bytes INTEGER NOT NULL DEFAULT 0,
              deleted INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS ocr_results (
              entry_id INTEGER PRIMARY KEY,
              status TEXT NOT NULL,
              text TEXT,
              language TEXT,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              FOREIGN KEY(entry_id) REFERENCES entries(id)
            );

            CREATE TABLE IF NOT EXISTS secrets (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              source_entry_id INTEGER UNIQUE,
              alias TEXT NOT NULL,
              value TEXT NOT NULL,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              last_used_at INTEGER,
              use_count INTEGER NOT NULL DEFAULT 0,
              deleted INTEGER NOT NULL DEFAULT 0,
              FOREIGN KEY(source_entry_id) REFERENCES entries(id)
            );

            CREATE INDEX IF NOT EXISTS idx_secrets_updated_at ON secrets(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_secrets_alias ON secrets(alias);
            "#,
        )?;
        self.ensure_columns(
            "entries",
            &[
                ("file_path", "TEXT"),
                ("thumb_path", "TEXT"),
                ("source_app", "TEXT"),
                ("link_url", "TEXT"),
                ("link_domain", "TEXT"),
                ("link_icon", "TEXT"),
                ("color_value", "TEXT"),
                ("color_format", "TEXT"),
                ("pinned", "INTEGER NOT NULL DEFAULT 0"),
                ("copied_at", "INTEGER NOT NULL DEFAULT 0"),
                ("updated_at", "INTEGER NOT NULL DEFAULT 0"),
                ("last_used_at", "INTEGER"),
                ("use_count", "INTEGER NOT NULL DEFAULT 0"),
                ("size_bytes", "INTEGER NOT NULL DEFAULT 0"),
                ("deleted", "INTEGER NOT NULL DEFAULT 0"),
            ],
        )?;
        self.ensure_columns(
            "secrets",
            &[
                ("last_used_at", "INTEGER"),
                ("use_count", "INTEGER NOT NULL DEFAULT 0"),
                ("deleted", "INTEGER NOT NULL DEFAULT 0"),
            ],
        )?;
        self.conn.execute_batch(
            r#"
            CREATE UNIQUE INDEX IF NOT EXISTS idx_entries_hash ON entries(content_hash);
            CREATE INDEX IF NOT EXISTS idx_entries_copied_at ON entries(copied_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_pinned ON entries(pinned DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_updated_at ON entries(deleted, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_pinned_updated ON entries(deleted, pinned DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_kind_pinned_updated ON entries(deleted, kind, pinned DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_type_sort ON entries(deleted, kind ASC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_most_used ON entries(deleted, use_count DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_kind_used ON entries(deleted, kind, use_count DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_secrets_updated_at ON secrets(deleted, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_kind ON entries(kind);
            CREATE INDEX IF NOT EXISTS idx_entries_domain ON entries(link_domain);

            UPDATE entries
               SET kind = 'text',
                   title = substr(text_content, 1, 96),
                   preview_text = text_content,
                   link_url = NULL,
                   link_domain = NULL,
                   link_icon = NULL
             WHERE kind = 'link'
               AND text_content IS NOT NULL
               AND trim(text_content) != ''
               AND lower(trim(text_content)) NOT LIKE 'http://%'
               AND lower(trim(text_content)) NOT LIKE 'https://%';
            "#,
        )?;
        Ok(())
    }

    fn ensure_columns(&self, table: &str, columns: &[(&str, &str)]) -> Result<()> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let existing: std::collections::HashSet<String> = stmt
            .query_map([], |row| row.get::<_, String>("name"))?
            .collect::<rusqlite::Result<_>>()?;
        for (column, definition) in columns {
            if !existing.contains(*column) {
                self.conn.execute(
                    &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
                    params![],
                )?;
            }
        }
        Ok(())
    }
}
