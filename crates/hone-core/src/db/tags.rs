//! Tag, transaction-tag, and tag rule operations

use rusqlite::params;
use tracing::info;

use super::{parse_datetime, Database, DbConn};
use crate::error::Result;
use crate::models::*;

impl Database {
    /// Seed the 15 root tags and subscription children (idempotent - skips existing tags)
    pub fn seed_root_tags(&self) -> Result<()> {
        let conn = self.conn()?;

        // Root tags - colors only, no auto_patterns (rely on bank category mappings instead)
        let root_tags = [
            ("Income", Some("#22c55e"), None::<&str>),
            ("Housing", Some("#6366f1"), None::<&str>),
            ("Utilities", Some("#8b5cf6"), None::<&str>),
            ("Groceries", Some("#10b981"), None::<&str>),
            ("Dining", Some("#f59e0b"), None::<&str>),
            ("Transport", Some("#ef4444"), None::<&str>),
            ("Healthcare", Some("#ec4899"), None::<&str>),
            ("Shopping", Some("#14b8a6"), None::<&str>),
            ("Entertainment", Some("#f97316"), None::<&str>),
            ("Subscriptions", Some("#a855f7"), None::<&str>),
            ("Travel", Some("#06b6d4"), None::<&str>),
            ("Personal", Some("#84cc16"), None::<&str>),
            ("Education", Some("#8b5cf6"), None::<&str>),
            ("Pets", Some("#f472b6"), None::<&str>),
            ("Gifts", Some("#d946ef"), None::<&str>),
            ("Financial", Some("#64748b"), None::<&str>),
            ("Other", Some("#9ca3af"), None::<&str>),
        ];

        for (name, color, auto_patterns) in &root_tags {
            // Check if already exists (NULL parent_id needs special handling)
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM tags WHERE name = ? AND parent_id IS NULL",
                    params![name],
                    |_| Ok(true),
                )
                .unwrap_or(false);

            if !exists {
                conn.execute(
                    r#"
                    INSERT INTO tags (name, parent_id, color, auto_patterns)
                    VALUES (?, NULL, ?, ?)
                    "#,
                    params![name, color, auto_patterns],
                )?;
            }
        }

        // Get Subscriptions tag ID to create children
        let subscriptions_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM tags WHERE name = 'Subscriptions' AND parent_id IS NULL",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(parent_id) = subscriptions_id {
            // Subscription category children (no auto_patterns - rely on bank categories and Ollama)
            let sub_categories: [(&str, Option<&str>); 7] = [
                ("Streaming", None),
                ("Music", None),
                ("CloudStorage", None),
                ("News", None),
                ("Fitness", None),
                ("Gaming", None),
                ("Software", None),
            ];

            for (name, auto_patterns) in &sub_categories {
                let exists: bool = conn
                    .query_row(
                        "SELECT 1 FROM tags WHERE name = ? AND parent_id = ?",
                        params![name, parent_id],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);

                if !exists {
                    conn.execute(
                        r#"
                        INSERT INTO tags (name, parent_id, auto_patterns)
                        VALUES (?, ?, ?)
                        "#,
                        params![name, parent_id, auto_patterns],
                    )?;
                }
            }
        }

        // Get Transport tag ID to create children
        let transport_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM tags WHERE name = 'Transport' AND parent_id IS NULL",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(parent_id) = transport_id {
            // Transport category children (no auto_patterns - rely on bank categories)
            let transport_categories: [(&str, Option<&str>); 6] = [
                ("Gas", None),
                ("Rideshare", None),
                ("Parking", None),
                ("Transit", None),
                ("Tolls", None),
                ("Auto", None),
            ];

            for (name, auto_patterns) in &transport_categories {
                let exists: bool = conn
                    .query_row(
                        "SELECT 1 FROM tags WHERE name = ? AND parent_id = ?",
                        params![name, parent_id],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);

                if !exists {
                    conn.execute(
                        r#"
                        INSERT INTO tags (name, parent_id, auto_patterns)
                        VALUES (?, ?, ?)
                        "#,
                        params![name, parent_id, auto_patterns],
                    )?;
                }
            }
        }

        // Get Personal tag ID to create children
        let personal_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM tags WHERE name = 'Personal' AND parent_id IS NULL",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(parent_id) = personal_id {
            // Personal category children (no auto_patterns - rely on bank categories)
            let personal_categories: [(&str, Option<&str>); 2] =
                [("Fitness", None), ("Education", None)];

            for (name, auto_patterns) in &personal_categories {
                let exists: bool = conn
                    .query_row(
                        "SELECT 1 FROM tags WHERE name = ? AND parent_id = ?",
                        params![name, parent_id],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);

                if !exists {
                    conn.execute(
                        r#"
                        INSERT INTO tags (name, parent_id, auto_patterns)
                        VALUES (?, ?, ?)
                        "#,
                        params![name, parent_id, auto_patterns],
                    )?;
                }
            }
        }

        // Get Shopping tag ID to create children
        let shopping_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM tags WHERE name = 'Shopping' AND parent_id IS NULL",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(parent_id) = shopping_id {
            // Shopping category children (no auto_patterns - rely on bank categories)
            let shopping_categories: [(&str, Option<&str>); 5] = [
                ("Home & Garden", None),
                ("Auto Parts", None),
                ("Electronics", None),
                ("Clothing", None),
                ("General", None),
            ];

            for (name, auto_patterns) in &shopping_categories {
                let exists: bool = conn
                    .query_row(
                        "SELECT 1 FROM tags WHERE name = ? AND parent_id = ?",
                        params![name, parent_id],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);

                if !exists {
                    conn.execute(
                        r#"
                        INSERT INTO tags (name, parent_id, auto_patterns)
                        VALUES (?, ?, ?)
                        "#,
                        params![name, parent_id, auto_patterns],
                    )?;
                }
            }
        }

        // Get Financial tag ID to create children
        let financial_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM tags WHERE name = 'Financial' AND parent_id IS NULL",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(parent_id) = financial_id {
            // Financial category children - Fees is important for excluding from subscription detection
            let financial_categories: [(&str, Option<&str>); 3] = [
                ("Fees", None),      // Interest charges, late fees, annual fees, etc.
                ("Transfers", None), // Bank transfers, payments
                ("Taxes", None),     // Tax payments
            ];

            for (name, auto_patterns) in &financial_categories {
                let exists: bool = conn
                    .query_row(
                        "SELECT 1 FROM tags WHERE name = ? AND parent_id = ?",
                        params![name, parent_id],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);

                if !exists {
                    conn.execute(
                        r#"
                        INSERT INTO tags (name, parent_id, auto_patterns)
                        VALUES (?, ?, ?)
                        "#,
                        params![name, parent_id, auto_patterns],
                    )?;
                }
            }
        }

        info!("Seeded root tags and category children");
        Ok(())
    }

    /// Create a new tag
    pub fn create_tag(
        &self,
        name: &str,
        parent_id: Option<i64>,
        color: Option<&str>,
        icon: Option<&str>,
        auto_patterns: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn()?;

        // Validate parent exists if specified
        if let Some(pid) = parent_id {
            let exists: bool = conn
                .query_row("SELECT 1 FROM tags WHERE id = ?", params![pid], |_| {
                    Ok(true)
                })
                .unwrap_or(false);
            if !exists {
                return Err(crate::error::Error::Tag(format!(
                    "Parent tag {} does not exist",
                    pid
                )));
            }
        }

        conn.execute(
            r#"
            INSERT INTO tags (name, parent_id, color, icon, auto_patterns)
            VALUES (?, ?, ?, ?, ?)
            "#,
            params![name, parent_id, color, icon, auto_patterns],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get a tag by ID
    pub fn get_tag(&self, id: i64) -> Result<Option<Tag>> {
        let conn = self.conn()?;

        let tag = conn
            .query_row(
                r#"
                SELECT id, name, parent_id, color, icon, auto_patterns, created_at
                FROM tags WHERE id = ?
                "#,
                params![id],
                |row| {
                    let created_at_str: String = row.get(6)?;
                    Ok(Tag {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        parent_id: row.get(2)?,
                        color: row.get(3)?,
                        icon: row.get(4)?,
                        auto_patterns: row.get(5)?,
                        created_at: parse_datetime(&created_at_str),
                    })
                },
            )
            .ok();

        Ok(tag)
    }

    /// Get a tag by path (e.g., "Transport.Gas" or "Groceries")
    pub fn get_tag_by_path(&self, path: &str) -> Result<Option<Tag>> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return Ok(None);
        }

        let conn = self.conn()?;
        let mut current_parent_id: Option<i64> = None;
        let mut current_tag: Option<Tag> = None;

        for part in parts {
            let tag = if let Some(pid) = current_parent_id {
                conn.query_row(
                    "SELECT id, name, parent_id, color, icon, auto_patterns, created_at FROM tags WHERE name = ? AND parent_id = ?",
                    params![part, pid],
                    |row| {
                        let created_at_str: String = row.get(6)?;
                        Ok(Tag {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            parent_id: row.get(2)?,
                            color: row.get(3)?,
                            icon: row.get(4)?,
                            auto_patterns: row.get(5)?,
                            created_at: parse_datetime(&created_at_str),
                        })
                    },
                )
                .ok()
            } else {
                conn.query_row(
                    "SELECT id, name, parent_id, color, icon, auto_patterns, created_at FROM tags WHERE name = ? AND parent_id IS NULL",
                    params![part],
                    |row| {
                        let created_at_str: String = row.get(6)?;
                        Ok(Tag {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            parent_id: row.get(2)?,
                            color: row.get(3)?,
                            icon: row.get(4)?,
                            auto_patterns: row.get(5)?,
                            created_at: parse_datetime(&created_at_str),
                        })
                    },
                )
                .ok()
            };

            match tag {
                Some(t) => {
                    current_parent_id = Some(t.id);
                    current_tag = Some(t);
                }
                None => return Ok(None),
            }
        }

        Ok(current_tag)
    }

    /// Update a tag
    pub fn update_tag(
        &self,
        id: i64,
        name: Option<&str>,
        parent_id: Option<Option<i64>>,
        color: Option<Option<&str>>,
        icon: Option<Option<&str>>,
        auto_patterns: Option<Option<&str>>,
    ) -> Result<()> {
        let conn = self.conn()?;

        // Build dynamic update query
        let mut updates = Vec::new();
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(n) = name {
            updates.push("name = ?");
            values.push(Box::new(n.to_string()));
        }
        if let Some(pid) = parent_id {
            updates.push("parent_id = ?");
            values.push(Box::new(pid));
        }
        if let Some(c) = color {
            updates.push("color = ?");
            values.push(Box::new(c.map(|s| s.to_string())));
        }
        if let Some(i) = icon {
            updates.push("icon = ?");
            values.push(Box::new(i.map(|s| s.to_string())));
        }
        if let Some(ap) = auto_patterns {
            updates.push("auto_patterns = ?");
            values.push(Box::new(ap.map(|s| s.to_string())));
        }

        if updates.is_empty() {
            return Ok(());
        }

        values.push(Box::new(id));
        let sql = format!("UPDATE tags SET {} WHERE id = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> = values.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())?;

        Ok(())
    }

    /// Delete a tag (returns transactions moved and children affected)
    pub fn delete_tag(&self, id: i64, reparent_to_parent: bool) -> Result<DeleteTagResult> {
        let conn = self.conn()?;

        // Get the tag's parent_id for reparenting
        let parent_id: Option<i64> = conn
            .query_row(
                "SELECT parent_id FROM tags WHERE id = ?",
                params![id],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        // Count affected transactions
        let transactions_moved: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transaction_tags WHERE tag_id = ?",
            params![id],
            |row| row.get(0),
        )?;

        // Count children
        let children_affected: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tags WHERE parent_id = ?",
            params![id],
            |row| row.get(0),
        )?;

        // Use explicit transaction for atomicity
        conn.execute("BEGIN TRANSACTION", [])?;

        let result = (|| {
            if reparent_to_parent {
                // Move transactions to parent tag (or remove if no parent)
                if let Some(pid) = parent_id {
                    conn.execute(
                        r#"
                        INSERT OR REPLACE INTO transaction_tags (transaction_id, tag_id, source, confidence)
                        SELECT transaction_id, ?, source, confidence
                        FROM transaction_tags WHERE tag_id = ?
                        "#,
                        params![pid, id],
                    )?;
                }

                // Reparent children to grandparent
                conn.execute(
                    "UPDATE tags SET parent_id = ? WHERE parent_id = ?",
                    params![parent_id, id],
                )?;
            }

            // Delete the tag (cascades to transaction_tags and tag_rules)
            conn.execute("DELETE FROM tags WHERE id = ?", params![id])?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                Ok(DeleteTagResult {
                    deleted_tag_id: id,
                    transactions_moved,
                    children_affected,
                })
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    /// List all tags (flat list)
    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, parent_id, color, icon, auto_patterns, created_at
            FROM tags ORDER BY name
            "#,
        )?;

        let tags = stmt
            .query_map([], |row| {
                let created_at_str: String = row.get(6)?;
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    color: row.get(3)?,
                    icon: row.get(4)?,
                    auto_patterns: row.get(5)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tags)
    }

    /// List root tags only
    pub fn list_root_tags(&self) -> Result<Vec<Tag>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, parent_id, color, icon, auto_patterns, created_at
            FROM tags WHERE parent_id IS NULL ORDER BY name
            "#,
        )?;

        let tags = stmt
            .query_map([], |row| {
                let created_at_str: String = row.get(6)?;
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    color: row.get(3)?,
                    icon: row.get(4)?,
                    auto_patterns: row.get(5)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tags)
    }

    /// Get children of a tag
    pub fn get_tag_children(&self, parent_id: i64) -> Result<Vec<Tag>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, parent_id, color, icon, auto_patterns, created_at
            FROM tags WHERE parent_id = ? ORDER BY name
            "#,
        )?;

        let tags = stmt
            .query_map(params![parent_id], |row| {
                let created_at_str: String = row.get(6)?;
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    color: row.get(3)?,
                    icon: row.get(4)?,
                    auto_patterns: row.get(5)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tags)
    }

    /// Build the tag path for a tag
    fn build_tag_path(&self, conn: &DbConn, tag_id: i64) -> Result<String> {
        let mut path_parts = Vec::new();
        let mut current_id = Some(tag_id);

        while let Some(id) = current_id {
            let (name, parent_id): (String, Option<i64>) = conn.query_row(
                "SELECT name, parent_id FROM tags WHERE id = ?",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            path_parts.push(name);
            current_id = parent_id;
        }

        path_parts.reverse();
        Ok(path_parts.join("."))
    }

    /// Get the full tag tree
    pub fn get_tag_tree(&self) -> Result<Vec<TagWithPath>> {
        let conn = self.conn()?;

        // Get all tags
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, parent_id, color, icon, auto_patterns, created_at
            FROM tags ORDER BY name
            "#,
        )?;

        let all_tags: Vec<Tag> = stmt
            .query_map([], |row| {
                let created_at_str: String = row.get(6)?;
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    color: row.get(3)?,
                    icon: row.get(4)?,
                    auto_patterns: row.get(5)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Build tree structure
        fn build_subtree(
            tags: &[Tag],
            parent_id: Option<i64>,
            parent_path: &str,
            depth: i32,
        ) -> Vec<TagWithPath> {
            tags.iter()
                .filter(|t| t.parent_id == parent_id)
                .map(|t| {
                    let path = if parent_path.is_empty() {
                        t.name.clone()
                    } else {
                        format!("{}.{}", parent_path, t.name)
                    };
                    let children = build_subtree(tags, Some(t.id), &path, depth + 1);
                    TagWithPath {
                        tag: t.clone(),
                        path,
                        depth,
                        children,
                    }
                })
                .collect()
        }

        Ok(build_subtree(&all_tags, None, "", 0))
    }

    /// Check if a tag name is ambiguous (exists at multiple levels)
    pub fn is_tag_name_ambiguous(&self, name: &str) -> Result<bool> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tags WHERE name = ?",
            params![name],
            |row| row.get(0),
        )?;
        Ok(count > 1)
    }

    /// Resolve a tag name to a tag (returns error if ambiguous)
    pub fn resolve_tag(&self, name: &str) -> Result<Option<Tag>> {
        // First try exact path match (before acquiring connection)
        if name.contains('.') {
            return self.get_tag_by_path(name);
        }

        let conn = self.conn()?;

        // Check for ambiguity
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tags WHERE name = ?",
            params![name],
            |row| row.get(0),
        )?;

        if count > 1 {
            return Err(crate::error::Error::Tag(format!(
                "Tag name '{}' is ambiguous ({} matches). Use full path.",
                name, count
            )));
        }

        let tag = conn
            .query_row(
                r#"
                SELECT id, name, parent_id, color, icon, auto_patterns, created_at
                FROM tags WHERE name = ?
                "#,
                params![name],
                |row| {
                    let created_at_str: String = row.get(6)?;
                    Ok(Tag {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        parent_id: row.get(2)?,
                        color: row.get(3)?,
                        icon: row.get(4)?,
                        auto_patterns: row.get(5)?,
                        created_at: parse_datetime(&created_at_str),
                    })
                },
            )
            .ok();

        Ok(tag)
    }

    /// Merge one tag into another (moves all transactions, then deletes source)
    pub fn merge_tags(&self, source_id: i64, target_id: i64) -> Result<i64> {
        let conn = self.conn()?;

        // Move all transaction_tags from source to target
        let moved: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transaction_tags WHERE tag_id = ?",
            params![source_id],
            |row| row.get(0),
        )?;

        // Use explicit transaction for atomicity
        conn.execute("BEGIN TRANSACTION", [])?;

        let result = (|| {
            conn.execute(
                r#"
                INSERT OR REPLACE INTO transaction_tags (transaction_id, tag_id, source, confidence, created_at)
                SELECT transaction_id, ?, source, confidence, created_at
                FROM transaction_tags WHERE tag_id = ?
                "#,
                params![target_id, source_id],
            )?;

            // Delete source tag (cascades delete remaining transaction_tags)
            conn.execute("DELETE FROM tags WHERE id = ?", params![source_id])?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                Ok(moved)
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    // ========== Transaction-Tag Operations ==========

    /// Add a tag to a transaction
    pub fn add_transaction_tag(
        &self,
        transaction_id: i64,
        tag_id: i64,
        source: TagSource,
        confidence: Option<f64>,
    ) -> Result<()> {
        let conn = self.conn()?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO transaction_tags (transaction_id, tag_id, source, confidence)
            VALUES (?, ?, ?, ?)
            "#,
            params![transaction_id, tag_id, source.as_str(), confidence],
        )?;

        Ok(())
    }

    /// Remove a tag from a transaction
    pub fn remove_transaction_tag(&self, transaction_id: i64, tag_id: i64) -> Result<()> {
        let conn = self.conn()?;

        conn.execute(
            "DELETE FROM transaction_tags WHERE transaction_id = ? AND tag_id = ?",
            params![transaction_id, tag_id],
        )?;

        Ok(())
    }

    /// Get all tags for a transaction
    pub fn get_transaction_tags(&self, transaction_id: i64) -> Result<Vec<TransactionTag>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT transaction_id, tag_id, source, confidence, created_at
            FROM transaction_tags WHERE transaction_id = ?
            "#,
        )?;

        let tags = stmt
            .query_map(params![transaction_id], |row| {
                let source_str: String = row.get(2)?;
                let created_at_str: String = row.get(4)?;
                Ok(TransactionTag {
                    transaction_id: row.get(0)?,
                    tag_id: row.get(1)?,
                    source: source_str.parse().unwrap_or(TagSource::Manual),
                    confidence: row.get(3)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tags)
    }

    /// Get all tags for a transaction with full tag details (for API responses)
    pub fn get_transaction_tags_with_details(
        &self,
        transaction_id: i64,
    ) -> Result<Vec<TransactionTagWithDetails>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            WITH RECURSIVE tag_path AS (
                SELECT id, name, parent_id, color,
                       name as path
                FROM tags WHERE parent_id IS NULL
                UNION ALL
                SELECT t.id, t.name, t.parent_id, t.color,
                       tp.path || '.' || t.name
                FROM tags t
                INNER JOIN tag_path tp ON t.parent_id = tp.id
            )
            SELECT tt.tag_id, tp.name, tp.path, tp.color, tt.source, tt.confidence
            FROM transaction_tags tt
            INNER JOIN tag_path tp ON tt.tag_id = tp.id
            WHERE tt.transaction_id = ?
            "#,
        )?;

        let tags = stmt
            .query_map(params![transaction_id], |row| {
                let source_str: String = row.get(4)?;
                Ok(TransactionTagWithDetails {
                    tag_id: row.get(0)?,
                    tag_name: row.get(1)?,
                    tag_path: row.get(2)?,
                    tag_color: row.get(3)?,
                    source: source_str.parse().unwrap_or(TagSource::Manual),
                    confidence: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tags)
    }

    /// Get all transactions with a specific tag (including descendants)
    pub fn get_transactions_by_tag(
        &self,
        tag_id: i64,
        include_descendants: bool,
    ) -> Result<Vec<Transaction>> {
        let conn = self.conn()?;

        let sql = if include_descendants {
            r#"
            WITH RECURSIVE tag_tree AS (
                SELECT id FROM tags WHERE id = ?
                UNION ALL
                SELECT t.id FROM tags t INNER JOIN tag_tree tt ON t.parent_id = tt.id
            )
            SELECT DISTINCT t.id, t.account_id, t.date, t.description, t.amount,
                   t.category, t.merchant_normalized, t.import_hash,
                   t.purchase_location_id, t.vendor_location_id, t.trip_id,
                   t.source, t.expected_amount, t.archived, t.original_data, t.import_format, t.card_member, t.payment_method, t.created_at
            FROM transactions t
            INNER JOIN transaction_tags tt ON t.id = tt.transaction_id
            WHERE tt.tag_id IN (SELECT id FROM tag_tree) AND t.archived = 0
            ORDER BY t.date DESC
            "#
        } else {
            r#"
            SELECT t.id, t.account_id, t.date, t.description, t.amount,
                   t.category, t.merchant_normalized, t.import_hash,
                   t.purchase_location_id, t.vendor_location_id, t.trip_id,
                   t.source, t.expected_amount, t.archived, t.original_data, t.import_format, t.card_member, t.payment_method, t.created_at
            FROM transactions t
            INNER JOIN transaction_tags tt ON t.id = tt.transaction_id
            WHERE tt.tag_id = ? AND t.archived = 0
            ORDER BY t.date DESC
            "#
        };

        let mut stmt = conn.prepare(sql)?;
        let transactions = stmt
            .query_map(params![tag_id], |row| Self::row_to_transaction(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(transactions)
    }

    /// Count transactions by tag
    pub fn count_transactions_by_tag(&self, tag_id: i64) -> Result<i64> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transaction_tags WHERE tag_id = ?",
            params![tag_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Clear all auto-assigned tags for a set of transactions (keeps manual tags)
    /// Returns the number of tags removed
    pub fn clear_auto_tags_for_transactions(&self, transaction_ids: &[i64]) -> Result<usize> {
        if transaction_ids.is_empty() {
            return Ok(0);
        }

        let conn = self.conn()?;

        // Build placeholders for IN clause
        let placeholders: Vec<String> = transaction_ids.iter().map(|_| "?".to_string()).collect();
        let placeholders_str = placeholders.join(", ");

        let sql = format!(
            "DELETE FROM transaction_tags WHERE transaction_id IN ({}) AND source != 'manual'",
            placeholders_str
        );

        let mut stmt = conn.prepare(&sql)?;

        // Convert transaction_ids to params
        let params: Vec<&dyn rusqlite::ToSql> = transaction_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();

        let deleted = stmt.execute(params.as_slice())?;

        info!(
            "Cleared {} auto-assigned tags from {} transactions",
            deleted,
            transaction_ids.len()
        );

        Ok(deleted)
    }

    /// Get all transaction IDs that have a specific tag (including descendants)
    /// Useful for efficiently filtering transactions in detection algorithms
    pub fn get_transaction_ids_with_tag(
        &self,
        tag_id: i64,
    ) -> Result<std::collections::HashSet<i64>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            WITH RECURSIVE tag_tree AS (
                SELECT id FROM tags WHERE id = ?
                UNION ALL
                SELECT t.id FROM tags t INNER JOIN tag_tree tt ON t.parent_id = tt.id
            )
            SELECT DISTINCT tt.transaction_id
            FROM transaction_tags tt
            WHERE tt.tag_id IN (SELECT id FROM tag_tree)
            "#,
        )?;

        let ids = stmt
            .query_map(params![tag_id], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<std::collections::HashSet<_>, _>>()?;

        Ok(ids)
    }

    /// Get untagged transactions
    pub fn get_untagged_transactions(&self, limit: i64) -> Result<Vec<Transaction>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT t.id, t.account_id, t.date, t.description, t.amount,
                   t.category, t.merchant_normalized, t.import_hash,
                   t.purchase_location_id, t.vendor_location_id, t.trip_id,
                   t.source, t.expected_amount, t.archived, t.original_data, t.import_format, t.card_member, t.payment_method, t.created_at
            FROM transactions t
            LEFT JOIN transaction_tags tt ON t.id = tt.transaction_id
            WHERE tt.transaction_id IS NULL AND t.archived = 0
            ORDER BY t.date DESC
            LIMIT ?
            "#,
        )?;

        let transactions = stmt
            .query_map(params![limit], |row| Self::row_to_transaction(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(transactions)
    }

    /// Get untagged transactions for a specific import session
    pub fn get_untagged_transactions_for_session(
        &self,
        session_id: i64,
    ) -> Result<Vec<Transaction>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT t.id, t.account_id, t.date, t.description, t.amount,
                   t.category, t.merchant_normalized, t.import_hash,
                   t.purchase_location_id, t.vendor_location_id, t.trip_id,
                   t.source, t.expected_amount, t.archived, t.original_data, t.import_format, t.card_member, t.payment_method, t.created_at
            FROM transactions t
            LEFT JOIN transaction_tags tt ON t.id = tt.transaction_id
            WHERE tt.transaction_id IS NULL AND t.archived = 0 AND t.import_session_id = ?
            ORDER BY t.date DESC
            "#,
        )?;

        let transactions = stmt
            .query_map(params![session_id], |row| Self::row_to_transaction(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(transactions)
    }

    // ========== Tag Rule Operations ==========

    /// Create a tag rule
    pub fn create_tag_rule(
        &self,
        tag_id: i64,
        pattern: &str,
        pattern_type: PatternType,
        priority: i32,
    ) -> Result<i64> {
        let conn = self.conn()?;

        conn.execute(
            r#"
            INSERT INTO tag_rules (tag_id, pattern, pattern_type, priority)
            VALUES (?, ?, ?, ?)
            "#,
            params![tag_id, pattern, pattern_type.as_str(), priority],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Delete a tag rule
    pub fn delete_tag_rule(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM tag_rules WHERE id = ?", params![id])?;
        Ok(())
    }

    /// List all tag rules
    pub fn list_tag_rules(&self) -> Result<Vec<TagRuleWithTag>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT r.id, r.tag_id, r.pattern, r.pattern_type, r.priority, r.created_at,
                   t.name
            FROM tag_rules r
            INNER JOIN tags t ON r.tag_id = t.id
            ORDER BY r.priority DESC, r.created_at
            "#,
        )?;

        let rules = stmt
            .query_map([], |row| {
                let pattern_type_str: String = row.get(3)?;
                let created_at_str: String = row.get(5)?;
                let tag_id: i64 = row.get(1)?;

                Ok((
                    TagRule {
                        id: row.get(0)?,
                        tag_id,
                        pattern: row.get(2)?,
                        pattern_type: pattern_type_str.parse().unwrap_or(PatternType::Contains),
                        priority: row.get(4)?,
                        created_at: parse_datetime(&created_at_str),
                    },
                    row.get::<_, String>(6)?,
                    tag_id,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Build full paths for each rule
        let mut result = Vec::new();
        for (rule, tag_name, tag_id) in rules {
            let path = self.build_tag_path(&conn, tag_id)?;
            result.push(TagRuleWithTag {
                rule,
                tag_name,
                tag_path: path,
            });
        }

        Ok(result)
    }

    /// Get rules for a specific tag
    pub fn get_tag_rules(&self, tag_id: i64) -> Result<Vec<TagRule>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, tag_id, pattern, pattern_type, priority, created_at
            FROM tag_rules WHERE tag_id = ?
            ORDER BY priority DESC
            "#,
        )?;

        let rules = stmt
            .query_map(params![tag_id], |row| {
                let pattern_type_str: String = row.get(3)?;
                let created_at_str: String = row.get(5)?;
                Ok(TagRule {
                    id: row.get(0)?,
                    tag_id: row.get(1)?,
                    pattern: row.get(2)?,
                    pattern_type: pattern_type_str.parse().unwrap_or(PatternType::Contains),
                    priority: row.get(4)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rules)
    }

    // ========== Tag Reporting ==========

    /// Get spending by tag with recursive rollup
    pub fn get_spending_by_tag(
        &self,
        from_date: Option<chrono::NaiveDate>,
        to_date: Option<chrono::NaiveDate>,
    ) -> Result<Vec<TagSpending>> {
        let conn = self.conn()?;

        // Build date filter with parameterized queries
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(f) = from_date {
            conditions.push("t.date >= ?");
            params.push(Box::new(f.to_string()));
        }
        if let Some(t) = to_date {
            conditions.push("t.date <= ?");
            params.push(Box::new(t.to_string()));
        }

        let date_filter = if conditions.is_empty() {
            String::new()
        } else {
            format!("AND {}", conditions.join(" AND "))
        };

        let sql = format!(
            r#"
            WITH RECURSIVE tag_tree AS (
                SELECT id, name, parent_id, id as root_id FROM tags WHERE parent_id IS NULL
                UNION ALL
                SELECT t.id, t.name, t.parent_id, tt.root_id
                FROM tags t INNER JOIN tag_tree tt ON t.parent_id = tt.id
            ),
            direct_spending AS (
                SELECT tt.tag_id,
                       SUM(CASE WHEN t.amount < 0 THEN ABS(t.amount) ELSE 0 END) as amount,
                       COUNT(*) as tx_count
                FROM transaction_tags tt
                INNER JOIN transactions t ON tt.transaction_id = t.id
                WHERE 1=1 {}
                GROUP BY tt.tag_id
            )
            SELECT tags.id, tags.name,
                   COALESCE(ds.amount, 0) as direct_amount,
                   COALESCE((
                       SELECT SUM(ds2.amount)
                       FROM tag_tree tt2
                       INNER JOIN direct_spending ds2 ON tt2.id = ds2.tag_id
                       WHERE tt2.root_id = tags.id OR tt2.id = tags.id
                   ), 0) as total_amount,
                   COALESCE(ds.tx_count, 0) as tx_count
            FROM tags
            LEFT JOIN direct_spending ds ON tags.id = ds.tag_id
            WHERE tags.parent_id IS NULL
            ORDER BY total_amount DESC
            "#,
            date_filter
        );

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut spending: Vec<TagSpending> = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(TagSpending {
                    tag_id: row.get(0)?,
                    tag_name: row.get(1)?,
                    tag_path: row.get::<_, String>(1)?, // Root tags have path = name
                    direct_amount: row.get(2)?,
                    total_amount: row.get(3)?,
                    transaction_count: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Filter out zero spending
        spending.retain(|s| s.total_amount > 0.0 || s.transaction_count > 0);

        Ok(spending)
    }

    /// Get subscription category tags (children of Subscriptions root tag)
    /// Returns tags with their auto_patterns for matching
    pub fn get_subscription_categories(&self) -> Result<Vec<Tag>> {
        let conn = self.conn()?;

        // Find Subscriptions root tag children directly
        let mut stmt = conn.prepare(
            r#"
            SELECT t.id, t.name, t.parent_id, t.color, t.icon, t.auto_patterns, t.created_at
            FROM tags t
            INNER JOIN tags p ON t.parent_id = p.id
            WHERE p.name = 'Subscriptions' AND p.parent_id IS NULL
            ORDER BY t.name
            "#,
        )?;

        let tags = stmt
            .query_map([], |row| {
                let created_at_str: String = row.get(6)?;
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    color: row.get(3)?,
                    icon: row.get(4)?,
                    auto_patterns: row.get(5)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tags)
    }

    /// Find which subscription category a merchant matches based on tag auto_patterns
    /// Returns the matching tag name (e.g., "Streaming", "Music")
    pub fn categorize_merchant_by_tags(&self, merchant: &str) -> Result<Option<String>> {
        let categories = self.get_subscription_categories()?;
        let merchant_upper = merchant.to_uppercase();

        for tag in categories {
            if let Some(ref patterns) = tag.auto_patterns {
                // Check if any pattern matches (pipe-separated, case-insensitive)
                for pattern in patterns.split('|') {
                    if merchant_upper.contains(&pattern.to_uppercase()) {
                        return Ok(Some(tag.name));
                    }
                }
            }
        }

        Ok(None)
    }

    // ============================================
    // Merchant-Tag Learning Cache
    // ============================================

    /// Look up a cached tag assignment for a merchant/description
    /// Returns (tag_id, tag_name, confidence) if found
    pub fn get_cached_merchant_tag(&self, description: &str) -> Result<Option<(i64, String, f64)>> {
        let conn = self.conn()?;

        // Look up by exact description match first
        let result: Option<(i64, String, f64)> = conn
            .query_row(
                r#"
                SELECT mtc.tag_id, t.name, mtc.confidence
                FROM merchant_tag_cache mtc
                INNER JOIN tags t ON mtc.tag_id = t.id
                WHERE mtc.merchant_pattern = ?
                ORDER BY mtc.confidence DESC
                LIMIT 1
                "#,
                params![description],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        // Increment hit count if found
        if result.is_some() {
            let _ = conn.execute(
                r#"
                UPDATE merchant_tag_cache
                SET hit_count = hit_count + 1, updated_at = CURRENT_TIMESTAMP
                WHERE merchant_pattern = ?
                "#,
                params![description],
            );
        }

        Ok(result)
    }

    /// Cache a merchant→tag association (from user's manual tag assignment)
    pub fn cache_merchant_tag(
        &self,
        merchant_pattern: &str,
        tag_id: i64,
        source: &str,
        confidence: f64,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            r#"
            INSERT INTO merchant_tag_cache (merchant_pattern, tag_id, source, confidence)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(merchant_pattern, tag_id) DO UPDATE SET
                source = excluded.source,
                confidence = excluded.confidence,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![merchant_pattern, tag_id, source, confidence],
        )?;
        Ok(())
    }

    /// Learn a tag association from a manual tag assignment
    /// Called when user manually tags a transaction
    pub fn learn_tag_from_manual_assignment(&self, transaction_id: i64, tag_id: i64) -> Result<()> {
        let conn = self.conn()?;

        // Get the transaction's description (the key we learn on)
        let description: String = conn.query_row(
            "SELECT description FROM transactions WHERE id = ?",
            params![transaction_id],
            |row| row.get(0),
        )?;

        // Cache the merchant→tag association with high confidence (user correction)
        self.cache_merchant_tag(&description, tag_id, "user", 1.0)?;

        Ok(())
    }

    /// Get merchant-tag cache statistics
    pub fn get_merchant_tag_cache_stats(&self) -> Result<(i64, i64, i64)> {
        let conn = self.conn()?;

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM merchant_tag_cache", [], |row| {
            row.get(0)
        })?;

        let user_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM merchant_tag_cache WHERE source = 'user'",
            [],
            |row| row.get(0),
        )?;

        let total_hits: i64 = conn.query_row(
            "SELECT COALESCE(SUM(hit_count), 0) FROM merchant_tag_cache",
            [],
            |row| row.get(0),
        )?;

        Ok((total, user_count, total_hits))
    }
}
