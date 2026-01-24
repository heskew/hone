//! Spending reports and analytics

use chrono::{Datelike, NaiveDate};
use rusqlite::params;

use super::{parse_datetime, Database, DbConn};
use crate::error::Result;
use crate::models::*;

impl Database {
    /// Get spending summary for a date range, grouped by root-level tags
    /// Returns categories with amounts and percentages, plus untagged summary
    /// entity_id filters by account owner, card_member filters by cardholder name
    pub fn get_spending_summary(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        tag_filter: Option<&str>,
        expand: bool,
        entity_id: Option<i64>,
        card_member: Option<&str>,
    ) -> Result<SpendingSummary> {
        use crate::models::{ReportPeriod, SpendingSummary, UntaggedSummary};
        let conn = self.conn()?;

        // Build filter clauses for entity/card_member
        let (extra_join, extra_where) =
            self.build_entity_card_filter_clauses(entity_id, card_member);

        // Get total spending in period (expenses only, negative amounts, exclude archived)
        let total_sql = format!(
            "SELECT COALESCE(SUM(ABS(t.amount)), 0) FROM transactions t {} WHERE t.amount < 0 AND t.archived = 0 AND t.date BETWEEN ?1 AND ?2 {}",
            extra_join, extra_where
        );
        let mut total_params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(from.to_string()), Box::new(to.to_string())];
        self.append_entity_card_params(&mut total_params, entity_id, card_member);
        let total_refs: Vec<&dyn rusqlite::ToSql> =
            total_params.iter().map(|p| p.as_ref()).collect();
        let total: f64 = conn.query_row(&total_sql, total_refs.as_slice(), |row| row.get(0))?;

        // Get untagged spending (exclude archived)
        let untagged_sql = format!(
            r#"
            SELECT COALESCE(SUM(ABS(t.amount)), 0), COUNT(*)
            FROM transactions t
            {}
            WHERE t.amount < 0
              AND t.archived = 0
              AND t.date BETWEEN ?1 AND ?2
              {}
              AND NOT EXISTS (SELECT 1 FROM transaction_tags tt WHERE tt.transaction_id = t.id)
            "#,
            extra_join, extra_where
        );
        let mut untagged_params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(from.to_string()), Box::new(to.to_string())];
        self.append_entity_card_params(&mut untagged_params, entity_id, card_member);
        let untagged_refs: Vec<&dyn rusqlite::ToSql> =
            untagged_params.iter().map(|p| p.as_ref()).collect();
        let (untagged_amount, untagged_count): (f64, i64) =
            conn.query_row(&untagged_sql, untagged_refs.as_slice(), |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;

        // Build query based on whether we're filtering by tag or expanding
        let categories = if let Some(tag_name) = tag_filter {
            // Filter to a specific tag and optionally expand its children
            self.get_category_spending_filtered(
                &conn,
                from,
                to,
                tag_name,
                expand,
                total,
                entity_id,
                card_member,
            )?
        } else {
            // Get root-level categories
            self.get_category_spending_roots(
                &conn,
                from,
                to,
                expand,
                total,
                entity_id,
                card_member,
            )?
        };

        Ok(SpendingSummary {
            period: ReportPeriod {
                from: from.to_string(),
                to: to.to_string(),
            },
            total,
            categories,
            untagged: UntaggedSummary {
                amount: untagged_amount,
                percentage: if total > 0.0 {
                    (untagged_amount / total) * 100.0
                } else {
                    0.0
                },
                transaction_count: untagged_count,
            },
        })
    }

    /// Helper: build entity/card_member filter SQL clauses (join and where parts)
    /// Uses "t" as the transaction table alias by default
    fn build_entity_card_filter_clauses(
        &self,
        entity_id: Option<i64>,
        card_member: Option<&str>,
    ) -> (String, String) {
        self.build_entity_card_filter_clauses_with_alias(entity_id, card_member, "t")
    }

    /// Helper: build entity/card_member filter SQL clauses with custom table alias
    fn build_entity_card_filter_clauses_with_alias(
        &self,
        entity_id: Option<i64>,
        card_member: Option<&str>,
        tx_alias: &str,
    ) -> (String, String) {
        let mut joins = Vec::new();
        let mut conditions = Vec::new();

        if entity_id.is_some() {
            joins.push(format!("JOIN accounts a ON {}.account_id = a.id", tx_alias));
            conditions.push("a.entity_id = ?".to_string());
        }

        if let Some(cm) = card_member {
            if !cm.trim().is_empty() {
                conditions.push(format!("{}.card_member = ? COLLATE NOCASE", tx_alias));
            }
        }

        let join_clause = joins.join(" ");
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("AND {}", conditions.join(" AND "))
        };

        (join_clause, where_clause)
    }

    /// Helper: append entity/card_member params to a params vector
    fn append_entity_card_params(
        &self,
        params: &mut Vec<Box<dyn rusqlite::ToSql>>,
        entity_id: Option<i64>,
        card_member: Option<&str>,
    ) {
        if let Some(eid) = entity_id {
            params.push(Box::new(eid));
        }
        if let Some(cm) = card_member {
            if !cm.trim().is_empty() {
                params.push(Box::new(cm.trim().to_string()));
            }
        }
    }

    /// Helper: get root-level category spending
    fn get_category_spending_roots(
        &self,
        conn: &DbConn,
        from: NaiveDate,
        to: NaiveDate,
        expand: bool,
        total: f64,
        entity_id: Option<i64>,
        card_member: Option<&str>,
    ) -> Result<Vec<CategorySpending>> {
        use crate::models::CategorySpending;

        // Build filter clauses (using "tx" alias for transactions)
        let (extra_join, extra_where) =
            self.build_entity_card_filter_clauses_with_alias(entity_id, card_member, "tx");

        let sql = format!(
            r#"
            WITH RECURSIVE tag_tree AS (
                SELECT id, id as root_id FROM tags WHERE parent_id IS NULL
                UNION ALL
                SELECT tags.id, tt.root_id FROM tags JOIN tag_tree tt ON tags.parent_id = tt.id
            )
            SELECT
                root.id,
                root.name,
                COALESCE(SUM(ABS(tx.amount)), 0) as amount,
                COUNT(DISTINCT tx.id) as tx_count
            FROM tags root
            LEFT JOIN tag_tree tt ON tt.root_id = root.id
            LEFT JOIN transaction_tags txg ON txg.tag_id = tt.id
            LEFT JOIN transactions tx ON tx.id = txg.transaction_id
                AND tx.amount < 0
                AND tx.archived = 0
                AND tx.date BETWEEN ?1 AND ?2
            {}
            WHERE root.parent_id IS NULL
            {}
            GROUP BY root.id, root.name
            HAVING amount > 0 OR tx_count > 0
            ORDER BY amount DESC
            "#,
            extra_join, extra_where
        );

        let mut stmt = conn.prepare(&sql)?;

        let mut query_params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(from.to_string()), Box::new(to.to_string())];
        self.append_entity_card_params(&mut query_params, entity_id, card_member);
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            query_params.iter().map(|p| p.as_ref()).collect();

        let mut categories: Vec<CategorySpending> = stmt
            .query_map(param_refs.as_slice(), |row| {
                let amount: f64 = row.get(2)?;
                Ok(CategorySpending {
                    tag_id: row.get(0)?,
                    tag: row.get(1)?,
                    amount,
                    percentage: if total > 0.0 {
                        (amount / total) * 100.0
                    } else {
                        0.0
                    },
                    transaction_count: row.get(3)?,
                    children: vec![],
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Expand children if requested
        if expand {
            for cat in &mut categories {
                cat.children = self.get_category_children(
                    conn,
                    from,
                    to,
                    cat.tag_id,
                    total,
                    entity_id,
                    card_member,
                )?;
            }
        }

        Ok(categories)
    }

    /// Helper: get spending filtered to a specific tag
    fn get_category_spending_filtered(
        &self,
        conn: &DbConn,
        from: NaiveDate,
        to: NaiveDate,
        tag_name: &str,
        expand: bool,
        total: f64,
        entity_id: Option<i64>,
        card_member: Option<&str>,
    ) -> Result<Vec<CategorySpending>> {
        use crate::models::CategorySpending;

        // Find the tag by name or path - use conn to avoid deadlock
        let tag = self.resolve_tag_with_conn(conn, tag_name)?;

        // Build filter clauses (using "tx" alias for transactions)
        let (extra_join, extra_where) =
            self.build_entity_card_filter_clauses_with_alias(entity_id, card_member, "tx");

        // Get spending for this tag and all descendants
        let sql = format!(
            r#"
            WITH RECURSIVE tag_tree AS (
                SELECT id FROM tags WHERE id = ?1
                UNION ALL
                SELECT tags.id FROM tags JOIN tag_tree tt ON tags.parent_id = tt.id
            )
            SELECT
                COALESCE(SUM(ABS(tx.amount)), 0) as amount,
                COUNT(DISTINCT tx.id) as tx_count
            FROM tag_tree tt
            LEFT JOIN transaction_tags txg ON txg.tag_id = tt.id
            LEFT JOIN transactions tx ON tx.id = txg.transaction_id
                AND tx.amount < 0
                AND tx.archived = 0
                AND tx.date BETWEEN ?2 AND ?3
            {}
            WHERE 1=1 {}
            "#,
            extra_join, extra_where
        );

        let mut stmt = conn.prepare(&sql)?;
        let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(tag.id),
            Box::new(from.to_string()),
            Box::new(to.to_string()),
        ];
        self.append_entity_card_params(&mut query_params, entity_id, card_member);
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            query_params.iter().map(|p| p.as_ref()).collect();

        let (amount, tx_count): (f64, i64) =
            stmt.query_row(param_refs.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?;

        let children = if expand {
            self.get_category_children(conn, from, to, tag.id, total, entity_id, card_member)?
        } else {
            vec![]
        };

        Ok(vec![CategorySpending {
            tag_id: tag.id,
            tag: tag.name,
            amount,
            percentage: if total > 0.0 {
                (amount / total) * 100.0
            } else {
                0.0
            },
            transaction_count: tx_count,
            children,
        }])
    }

    /// Helper: get child category spending for a parent tag
    fn get_category_children(
        &self,
        conn: &DbConn,
        from: NaiveDate,
        to: NaiveDate,
        parent_id: i64,
        total: f64,
        entity_id: Option<i64>,
        card_member: Option<&str>,
    ) -> Result<Vec<CategorySpending>> {
        use crate::models::CategorySpending;

        // Build filter clauses (using "tx" alias for transactions)
        let (extra_join, extra_where) =
            self.build_entity_card_filter_clauses_with_alias(entity_id, card_member, "tx");

        let sql = format!(
            r#"
            WITH RECURSIVE tag_tree AS (
                SELECT id, id as child_root_id FROM tags WHERE parent_id = ?1
                UNION ALL
                SELECT tags.id, tt.child_root_id FROM tags JOIN tag_tree tt ON tags.parent_id = tt.id
            )
            SELECT
                child.id,
                child.name,
                COALESCE(SUM(ABS(tx.amount)), 0) as amount,
                COUNT(DISTINCT tx.id) as tx_count
            FROM tags child
            LEFT JOIN tag_tree tt ON tt.child_root_id = child.id
            LEFT JOIN transaction_tags txg ON txg.tag_id = tt.id
            LEFT JOIN transactions tx ON tx.id = txg.transaction_id
                AND tx.amount < 0
                AND tx.archived = 0
                AND tx.date BETWEEN ?2 AND ?3
            {}
            WHERE child.parent_id = ?1 {}
            GROUP BY child.id, child.name
            HAVING amount > 0 OR tx_count > 0
            ORDER BY amount DESC
            "#,
            extra_join, extra_where
        );

        let mut stmt = conn.prepare(&sql)?;
        let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(parent_id),
            Box::new(from.to_string()),
            Box::new(to.to_string()),
        ];
        self.append_entity_card_params(&mut query_params, entity_id, card_member);
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            query_params.iter().map(|p| p.as_ref()).collect();

        let children: Vec<CategorySpending> = stmt
            .query_map(param_refs.as_slice(), |row| {
                let amount: f64 = row.get(2)?;
                Ok(CategorySpending {
                    tag_id: row.get(0)?,
                    tag: row.get(1)?,
                    amount,
                    percentage: if total > 0.0 {
                        (amount / total) * 100.0
                    } else {
                        0.0
                    },
                    transaction_count: row.get(3)?,
                    children: vec![],
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(children)
    }

    /// Helper: resolve a tag by name or path using an existing connection (to avoid deadlock)
    fn resolve_tag_with_conn(&self, conn: &DbConn, tag_name: &str) -> Result<crate::models::Tag> {
        if tag_name.contains('.') {
            // Path-based lookup
            let parts: Vec<&str> = tag_name.split('.').collect();
            let mut current_id: Option<i64> = None;
            for part in &parts {
                let result: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM tags WHERE name = ?1 AND parent_id IS ?2",
                        params![part, current_id],
                        |row| row.get(0),
                    )
                    .ok();
                match result {
                    Some(id) => current_id = Some(id),
                    None => {
                        return Err(crate::error::Error::NotFound(format!(
                            "Tag not found: {}",
                            tag_name
                        )))
                    }
                }
            }
            conn.query_row(
                "SELECT id, name, parent_id, color, icon, auto_patterns, created_at FROM tags WHERE id = ?1",
                params![current_id],
                |row| {
                    let created_at_str: String = row.get(6)?;
                    Ok(crate::models::Tag {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        parent_id: row.get(2)?,
                        color: row.get(3)?,
                        icon: row.get(4)?,
                        auto_patterns: row.get(5)?,
                        created_at: parse_datetime(&created_at_str),
                    })
                },
            ).map_err(|_| crate::error::Error::NotFound(format!("Tag not found: {}", tag_name)))
        } else {
            // Simple name lookup
            conn.query_row(
                "SELECT id, name, parent_id, color, icon, auto_patterns, created_at FROM tags WHERE name = ?1",
                params![tag_name],
                |row| {
                    let created_at_str: String = row.get(6)?;
                    Ok(crate::models::Tag {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        parent_id: row.get(2)?,
                        color: row.get(3)?,
                        icon: row.get(4)?,
                        auto_patterns: row.get(5)?,
                        created_at: parse_datetime(&created_at_str),
                    })
                },
            ).map_err(|_| crate::error::Error::NotFound(format!("Tag not found: {}", tag_name)))
        }
    }

    /// Get spending trends over time
    /// entity_id filters by account owner, card_member filters by cardholder name
    pub fn get_spending_trends(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        granularity: Granularity,
        tag_filter: Option<&str>,
        entity_id: Option<i64>,
        card_member: Option<&str>,
    ) -> Result<TrendsReport> {
        use crate::models::{Granularity, ReportPeriod, TrendDataPoint, TrendsReport};
        let conn = self.conn()?;

        // Build period grouping based on granularity
        let period_expr = match granularity {
            Granularity::Monthly => "strftime('%Y-%m', tx.date)",
            Granularity::Weekly => "strftime('%Y-W%W', tx.date)",
        };

        // Build filter clauses (using "tx" alias for transactions)
        let (extra_join, extra_where) =
            self.build_entity_card_filter_clauses_with_alias(entity_id, card_member, "tx");

        let (sql, tag_name) = if let Some(tag_name) = tag_filter {
            // Filter to specific tag and descendants - use conn to avoid deadlock
            let tag = self.resolve_tag_with_conn(&conn, tag_name)?;

            let sql = format!(
                r#"
                WITH RECURSIVE tag_tree AS (
                    SELECT id FROM tags WHERE id = ?1
                    UNION ALL
                    SELECT tags.id FROM tags JOIN tag_tree tt ON tags.parent_id = tt.id
                )
                SELECT
                    {} as period,
                    COALESCE(SUM(ABS(tx.amount)), 0) as amount,
                    COUNT(DISTINCT tx.id) as tx_count
                FROM transactions tx
                JOIN transaction_tags txg ON txg.transaction_id = tx.id
                JOIN tag_tree tt ON tt.id = txg.tag_id
                {}
                WHERE tx.amount < 0 AND tx.archived = 0 AND tx.date BETWEEN ?2 AND ?3 {}
                GROUP BY period
                ORDER BY period
                "#,
                period_expr, extra_join, extra_where
            );
            (sql, Some((tag.id, tag_name.to_string())))
        } else {
            // All spending
            let sql = format!(
                r#"
                SELECT
                    {} as period,
                    COALESCE(SUM(ABS(tx.amount)), 0) as amount,
                    COUNT(*) as tx_count
                FROM transactions tx
                {}
                WHERE tx.amount < 0 AND tx.archived = 0 AND tx.date BETWEEN ?1 AND ?2 {}
                GROUP BY period
                ORDER BY period
                "#,
                period_expr, extra_join, extra_where
            );
            (sql, None)
        };

        let mut stmt = conn.prepare(&sql)?;
        let data: Vec<TrendDataPoint> = if let Some((tag_id, _)) = &tag_name {
            let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = vec![
                Box::new(tag_id.clone()),
                Box::new(from.to_string()),
                Box::new(to.to_string()),
            ];
            self.append_entity_card_params(&mut query_params, entity_id, card_member);
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                query_params.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                Ok(TrendDataPoint {
                    period: row.get(0)?,
                    amount: row.get(1)?,
                    transaction_count: row.get(2)?,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            let mut query_params: Vec<Box<dyn rusqlite::ToSql>> =
                vec![Box::new(from.to_string()), Box::new(to.to_string())];
            self.append_entity_card_params(&mut query_params, entity_id, card_member);
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                query_params.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                Ok(TrendDataPoint {
                    period: row.get(0)?,
                    amount: row.get(1)?,
                    transaction_count: row.get(2)?,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        Ok(TrendsReport {
            granularity,
            period: ReportPeriod {
                from: from.to_string(),
                to: to.to_string(),
            },
            tag: tag_name.map(|(_, name)| name),
            data,
        })
    }

    /// Get top merchants by spending
    /// entity_id filters by account owner, card_member filters by cardholder name
    pub fn get_top_merchants(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        limit: i64,
        tag_filter: Option<&str>,
        entity_id: Option<i64>,
        card_member: Option<&str>,
    ) -> Result<MerchantsReport> {
        use crate::models::{MerchantSummary, MerchantsReport, ReportPeriod};
        let conn = self.conn()?;

        // Build filter clauses (using "tx" alias for transactions)
        let (extra_join, extra_where) =
            self.build_entity_card_filter_clauses_with_alias(entity_id, card_member, "tx");

        let merchants: Vec<MerchantSummary> = if let Some(tag_name) = tag_filter {
            // Use conn to avoid deadlock
            let tag = self.resolve_tag_with_conn(&conn, tag_name)?;

            // Count how many extra params we'll add for entity/card_member
            let extra_param_count = entity_id.map_or(0, |_| 1)
                + card_member
                    .filter(|cm| !cm.trim().is_empty())
                    .map_or(0, |_| 1);
            let limit_param_idx = 4 + extra_param_count;

            let sql = format!(
                r#"
                WITH RECURSIVE tag_tree AS (
                    SELECT id FROM tags WHERE id = ?1
                    UNION ALL
                    SELECT tags.id FROM tags JOIN tag_tree tt ON tags.parent_id = tt.id
                )
                SELECT
                    COALESCE(tx.merchant_normalized, tx.description) as merchant,
                    SUM(ABS(tx.amount)) as amount,
                    COUNT(*) as tx_count
                FROM transactions tx
                JOIN transaction_tags txg ON txg.transaction_id = tx.id
                JOIN tag_tree tt ON tt.id = txg.tag_id
                {}
                WHERE tx.amount < 0 AND tx.archived = 0 AND tx.date BETWEEN ?2 AND ?3 {}
                GROUP BY merchant
                ORDER BY amount DESC
                LIMIT ?{}
                "#,
                extra_join, extra_where, limit_param_idx
            );

            let mut stmt = conn.prepare(&sql)?;
            let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = vec![
                Box::new(tag.id),
                Box::new(from.to_string()),
                Box::new(to.to_string()),
            ];
            self.append_entity_card_params(&mut query_params, entity_id, card_member);
            query_params.push(Box::new(limit));
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                query_params.iter().map(|p| p.as_ref()).collect();

            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                Ok(MerchantSummary {
                    merchant: row.get(0)?,
                    amount: row.get(1)?,
                    transaction_count: row.get(2)?,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            // Count how many extra params we'll add for entity/card_member
            let extra_param_count = entity_id.map_or(0, |_| 1)
                + card_member
                    .filter(|cm| !cm.trim().is_empty())
                    .map_or(0, |_| 1);
            let limit_param_idx = 3 + extra_param_count;

            let sql = format!(
                r#"
                SELECT
                    COALESCE(tx.merchant_normalized, tx.description) as merchant,
                    SUM(ABS(tx.amount)) as amount,
                    COUNT(*) as tx_count
                FROM transactions tx
                {}
                WHERE tx.amount < 0 AND tx.archived = 0 AND tx.date BETWEEN ?1 AND ?2 {}
                GROUP BY merchant
                ORDER BY amount DESC
                LIMIT ?{}
                "#,
                extra_join, extra_where, limit_param_idx
            );

            let mut stmt = conn.prepare(&sql)?;
            let mut query_params: Vec<Box<dyn rusqlite::ToSql>> =
                vec![Box::new(from.to_string()), Box::new(to.to_string())];
            self.append_entity_card_params(&mut query_params, entity_id, card_member);
            query_params.push(Box::new(limit));
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                query_params.iter().map(|p| p.as_ref()).collect();

            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                Ok(MerchantSummary {
                    merchant: row.get(0)?,
                    amount: row.get(1)?,
                    transaction_count: row.get(2)?,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        Ok(MerchantsReport {
            period: ReportPeriod {
                from: from.to_string(),
                to: to.to_string(),
            },
            limit,
            merchants,
        })
    }

    /// Get subscription summary report
    pub fn get_subscription_summary(&self) -> Result<SubscriptionSummaryReport> {
        use crate::models::{SubscriptionInfo, SubscriptionSummaryReport};
        let conn = self.conn()?;

        // Get all subscriptions
        let mut stmt = conn.prepare(
            r#"
            SELECT id, merchant, amount, frequency, status, first_seen, last_seen
            FROM subscriptions
            ORDER BY CASE WHEN status = 'active' THEN 0 ELSE 1 END, amount DESC
            "#,
        )?;

        let subscriptions: Vec<SubscriptionInfo> = stmt
            .query_map([], |row| {
                Ok(SubscriptionInfo {
                    id: row.get(0)?,
                    merchant: row.get(1)?,
                    amount: row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                    frequency: row
                        .get::<_, Option<String>>(3)?
                        .unwrap_or_else(|| "monthly".to_string()),
                    status: row
                        .get::<_, Option<String>>(4)?
                        .unwrap_or_else(|| "active".to_string()),
                    first_seen: row.get(5)?,
                    last_seen: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Calculate totals
        let active_count = subscriptions
            .iter()
            .filter(|s| s.status == "active")
            .count() as i64;
        let cancelled_count = subscriptions
            .iter()
            .filter(|s| s.status == "cancelled")
            .count() as i64;

        // Calculate monthly total for active subscriptions (normalize frequencies)
        let total_monthly: f64 = subscriptions
            .iter()
            .filter(|s| s.status == "active")
            .map(|s| {
                match s.frequency.as_str() {
                    "weekly" => s.amount * 4.33,
                    "yearly" => s.amount / 12.0,
                    _ => s.amount, // monthly
                }
            })
            .sum();

        // Get waste breakdown from alerts
        let waste = self.get_waste_breakdown(&conn)?;

        Ok(SubscriptionSummaryReport {
            total_monthly,
            active_count,
            cancelled_count,
            subscriptions,
            waste,
        })
    }

    /// Helper: get waste breakdown from active alerts
    fn get_waste_breakdown(&self, conn: &DbConn) -> Result<WasteBreakdown> {
        use crate::models::WasteBreakdown;

        // Count zombies and their monthly cost
        let (zombie_count, zombie_monthly): (i64, f64) = conn.query_row(
            r#"
            SELECT COUNT(*), COALESCE(SUM(s.amount), 0)
            FROM alerts a
            JOIN subscriptions s ON s.id = a.subscription_id
            WHERE a.type = 'zombie' AND a.dismissed = 0
            "#,
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        // Count duplicates (each alert may cover multiple subs, so count alerts)
        let (duplicate_count, duplicate_monthly): (i64, f64) = conn.query_row(
            r#"
            SELECT COUNT(*), COALESCE(SUM(s.amount), 0)
            FROM alerts a
            LEFT JOIN subscriptions s ON s.id = a.subscription_id
            WHERE a.type = 'duplicate' AND a.dismissed = 0
            "#,
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        // Count price increases and delta
        let (price_increase_count, price_increase_delta): (i64, f64) = conn.query_row(
            r#"
            SELECT COUNT(*), 0.0
            FROM alerts
            WHERE type = 'price_increase' AND dismissed = 0
            "#,
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let total_waste_monthly = zombie_monthly + duplicate_monthly + price_increase_delta;

        Ok(WasteBreakdown {
            zombie_count,
            zombie_monthly,
            duplicate_count,
            duplicate_monthly,
            price_increase_count,
            price_increase_delta,
            total_waste_monthly,
        })
    }

    /// Cancel a subscription (mark as cancelled with date and monthly amount)
    pub fn cancel_subscription(&self, id: i64, cancelled_at: Option<NaiveDate>) -> Result<()> {
        let conn = self.conn()?;
        let cancel_date = cancelled_at.unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Get the current monthly amount before cancelling
        let monthly_amount: Option<f64> = conn
            .query_row(
                "SELECT amount FROM subscriptions WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .ok();

        conn.execute(
            r#"
            UPDATE subscriptions
            SET status = 'cancelled',
                cancelled_at = ?2,
                cancelled_monthly_amount = ?3
            WHERE id = ?1
            "#,
            params![id, cancel_date.to_string(), monthly_amount],
        )?;

        Ok(())
    }

    /// Get savings report from cancelled subscriptions
    pub fn get_savings_report(&self) -> Result<SavingsReport> {
        use crate::models::{CancelledSubscriptionInfo, SavingsReport};
        let conn = self.conn()?;
        let today = chrono::Utc::now().date_naive();
        let max_months = 12; // Cap savings at 12 months per REPORTS.md

        let mut stmt = conn.prepare(
            r#"
            SELECT id, merchant, cancelled_monthly_amount, cancelled_at
            FROM subscriptions
            WHERE status = 'cancelled' AND cancelled_at IS NOT NULL AND cancelled_monthly_amount > 0
            ORDER BY cancelled_at DESC
            "#,
        )?;

        let cancelled: Vec<CancelledSubscriptionInfo> = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let merchant: String = row.get(1)?;
                let monthly_amount: f64 = row.get(2)?;
                let cancelled_at_str: String = row.get(3)?;

                // Parse cancelled_at date
                let cancelled_at =
                    NaiveDate::parse_from_str(&cancelled_at_str, "%Y-%m-%d").unwrap_or(today);

                // Calculate months since cancellation
                let months_since = ((today.year() - cancelled_at.year()) * 12
                    + (today.month() as i32 - cancelled_at.month() as i32))
                    as i64;
                let months_counted = months_since.min(max_months).max(0);
                let months_remaining = (max_months - months_counted).max(0);
                let savings = monthly_amount * months_counted as f64;

                Ok(CancelledSubscriptionInfo {
                    id,
                    merchant,
                    monthly_amount,
                    cancelled_at: cancelled_at_str,
                    months_counted,
                    months_remaining,
                    savings,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let total_savings: f64 = cancelled.iter().map(|c| c.savings).sum();
        let total_monthly_saved: f64 = cancelled.iter().map(|c| c.monthly_amount).sum();
        let cancelled_count = cancelled.len() as i64;

        Ok(SavingsReport {
            total_savings,
            total_monthly_saved,
            cancelled_count,
            cancelled,
        })
    }

    /// Find subscription by merchant name or ID
    pub fn find_subscription_by_merchant_or_id(&self, name_or_id: &str) -> Result<Option<i64>> {
        let conn = self.conn()?;

        // First try to parse as ID
        if let Ok(id) = name_or_id.parse::<i64>() {
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM subscriptions WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )?;
            if exists {
                return Ok(Some(id));
            }
        }

        // Otherwise search by merchant name (case-insensitive partial match)
        let pattern = format!("%{}%", name_or_id.to_uppercase());
        let result = conn.query_row(
            "SELECT id FROM subscriptions WHERE UPPER(merchant) LIKE ?1 LIMIT 1",
            params![pattern],
            |row| row.get(0),
        );

        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
