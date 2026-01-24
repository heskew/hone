//! Transaction filter builder for constructing dynamic SQL queries
//!
//! This module provides a builder pattern for constructing WHERE clauses
//! and related SQL components for transaction queries.

use chrono::NaiveDate;

/// Builder for constructing transaction query filters
///
/// This avoids duplicating the query building logic between
/// `search_transactions_full` and `count_transactions_full`.
///
/// The lifetime `'query` represents how long the filter parameters
/// (search terms, card member name, tag IDs, etc.) must remain valid.
#[derive(Default)]
pub struct TransactionFilter<'query> {
    pub account_id: Option<i64>,
    pub entity_id: Option<i64>,
    pub card_member: Option<&'query str>,
    pub search: Option<&'query str>,
    pub tag_ids: Option<&'query [i64]>,
    pub untagged: bool,
    pub date_range: Option<(NaiveDate, NaiveDate)>,
    pub include_archived: bool,
    pub sort_field: Option<&'query str>,
    pub sort_order: Option<&'query str>,
}

/// Result of building a filter - contains SQL components and parameters
pub struct FilterResult {
    /// Optional CTE for tag hierarchy queries
    pub cte: Option<String>,
    /// JOIN clause (empty string if no joins needed)
    pub join_clause: &'static str,
    /// WHERE clause including "WHERE" keyword (empty if no conditions)
    pub where_clause: String,
    /// ORDER BY clause including "ORDER BY" keyword
    pub order_clause: String,
    /// Parameters for the query (boxed for rusqlite compatibility)
    pub params: Vec<Box<dyn rusqlite::ToSql>>,
}

impl<'query> TransactionFilter<'query> {
    /// Create a new filter builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set account_id filter
    pub fn account_id(mut self, id: Option<i64>) -> Self {
        self.account_id = id;
        self
    }

    /// Set entity_id filter (filters by account owner)
    pub fn entity_id(mut self, id: Option<i64>) -> Self {
        self.entity_id = id;
        self
    }

    /// Set card_member filter
    pub fn card_member(mut self, member: Option<&'query str>) -> Self {
        self.card_member = member;
        self
    }

    /// Set search query (searches description and merchant_normalized)
    pub fn search(mut self, query: Option<&'query str>) -> Self {
        self.search = query;
        self
    }

    /// Set tag_ids filter (includes descendant tags)
    pub fn tag_ids(mut self, ids: Option<&'query [i64]>) -> Self {
        self.tag_ids = ids;
        self
    }

    /// Set untagged filter (only transactions with no tags)
    pub fn untagged(mut self, value: bool) -> Self {
        self.untagged = value;
        self
    }

    /// Set date range filter
    pub fn date_range(mut self, range: Option<(NaiveDate, NaiveDate)>) -> Self {
        self.date_range = range;
        self
    }

    /// Set whether to include archived transactions
    pub fn include_archived(mut self, value: bool) -> Self {
        self.include_archived = value;
        self
    }

    /// Set sort field (date or amount)
    pub fn sort_field(mut self, field: Option<&'query str>) -> Self {
        self.sort_field = field;
        self
    }

    /// Set sort order (asc or desc)
    pub fn sort_order(mut self, order: Option<&'query str>) -> Self {
        self.sort_order = order;
        self
    }

    /// Build the filter components
    pub fn build(self) -> FilterResult {
        let mut conditions = Vec::new();
        // CTE params must come first since CTE appears before WHERE in SQL
        let mut cte_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut where_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // Determine if we need account join
        let needs_account_join = self.entity_id.is_some();

        // Tag filtering with hierarchy support (mutually exclusive with untagged)
        // IMPORTANT: CTE params must be collected first since CTE appears before WHERE clause
        let tag_cte = if !self.untagged {
            if let Some(ids) = self.tag_ids {
                if !ids.is_empty() {
                    let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
                    for id in ids {
                        cte_params.push(Box::new(*id));
                    }

                    let cte = format!(
                        r#"WITH RECURSIVE tag_tree AS (
                            SELECT id FROM tags WHERE id IN ({})
                            UNION ALL
                            SELECT t.id FROM tags t
                            INNER JOIN tag_tree tt ON t.parent_id = tt.id
                        )"#,
                        placeholders.join(", ")
                    );

                    conditions.push(
                        "t.id IN (SELECT transaction_id FROM transaction_tags WHERE tag_id IN (SELECT id FROM tag_tree))"
                            .to_string(),
                    );
                    Some(cte)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Account filter
        if let Some(aid) = self.account_id {
            conditions.push("t.account_id = ?".to_string());
            where_params.push(Box::new(aid));
        }

        // Entity filter (account owner)
        if let Some(eid) = self.entity_id {
            conditions.push("a.entity_id = ?".to_string());
            where_params.push(Box::new(eid));
        }

        // Card member filter
        if let Some(cm) = self.card_member {
            if !cm.trim().is_empty() {
                conditions.push("t.card_member = ? COLLATE NOCASE".to_string());
                where_params.push(Box::new(cm.trim().to_string()));
            }
        }

        // Search filter (description and merchant_normalized)
        if let Some(q) = self.search {
            if !q.trim().is_empty() {
                conditions.push(
                    "(t.description LIKE ? COLLATE NOCASE OR t.merchant_normalized LIKE ? COLLATE NOCASE)"
                        .to_string(),
                );
                let pattern = format!("%{}%", q.trim());
                where_params.push(Box::new(pattern.clone()));
                where_params.push(Box::new(pattern));
            }
        }

        // Date range filter
        if let Some((from_date, to_date)) = self.date_range {
            conditions.push("t.date >= ? AND t.date <= ?".to_string());
            where_params.push(Box::new(from_date.to_string()));
            where_params.push(Box::new(to_date.to_string()));
        }

        // Archived filter
        if !self.include_archived {
            conditions.push("t.archived = 0".to_string());
        }

        // Untagged filter
        if self.untagged {
            conditions
                .push("t.id NOT IN (SELECT transaction_id FROM transaction_tags)".to_string());
        }

        // Combine params: CTE params first, then WHERE params
        let mut params = cte_params;
        params.extend(where_params);

        // Build WHERE clause
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Build ORDER BY clause
        let order_column = match self.sort_field {
            Some("amount") => "t.amount",
            _ => "t.date",
        };
        let order_dir = match self.sort_order {
            Some("asc") => "ASC",
            _ => "DESC",
        };
        let order_clause = format!("ORDER BY {} {}, t.id DESC", order_column, order_dir);

        // Determine join clause
        let join_clause = if needs_account_join {
            "JOIN accounts a ON t.account_id = a.id"
        } else {
            ""
        };

        FilterResult {
            cte: tag_cte,
            join_clause,
            where_clause,
            order_clause,
            params,
        }
    }
}

impl FilterResult {
    /// Build a COUNT query
    pub fn build_count_query(&self) -> String {
        if let Some(ref cte) = self.cte {
            format!(
                "{} SELECT COUNT(*) FROM transactions t {} {}",
                cte, self.join_clause, self.where_clause
            )
        } else {
            format!(
                "SELECT COUNT(*) FROM transactions t {} {}",
                self.join_clause, self.where_clause
            )
        }
    }

    /// Get parameter references for query execution
    pub fn params_refs(&self) -> Vec<&dyn rusqlite::ToSql> {
        self.params.iter().map(|p| p.as_ref()).collect()
    }

    /// Get mutable parameter vector to append pagination params
    pub fn into_params(self) -> Vec<Box<dyn rusqlite::ToSql>> {
        self.params
    }
}
