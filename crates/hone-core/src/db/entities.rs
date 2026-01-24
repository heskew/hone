//! Entity, location, split, trip, and mileage operations

use chrono::NaiveDate;
use rusqlite::{params, OptionalExtension};

use super::{parse_datetime, Database};
use crate::error::{Error, Result};
use crate::models::*;

impl Database {
    /// Create a new entity
    pub fn create_entity(&self, entity: &NewEntity) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO entities (name, type, icon, color) VALUES (?, ?, ?, ?)",
            params![
                entity.name,
                entity.entity_type.as_str(),
                entity.icon,
                entity.color
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get an entity by ID
    pub fn get_entity(&self, id: i64) -> Result<Option<Entity>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, name, type, icon, color, archived, created_at FROM entities WHERE id = ?",
            params![id],
            |row| {
                let type_str: String = row.get(2)?;
                let created_at_str: String = row.get(6)?;
                Ok(Entity {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    entity_type: type_str.parse().unwrap_or(EntityType::Person),
                    icon: row.get(3)?,
                    color: row.get(4)?,
                    archived: row.get(5)?,
                    created_at: parse_datetime(&created_at_str),
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    /// List all entities, optionally including archived
    pub fn list_entities(&self, include_archived: bool) -> Result<Vec<Entity>> {
        let conn = self.conn()?;
        let query = if include_archived {
            "SELECT id, name, type, icon, color, archived, created_at FROM entities ORDER BY type, name"
        } else {
            "SELECT id, name, type, icon, color, archived, created_at FROM entities WHERE archived = 0 ORDER BY type, name"
        };

        let mut stmt = conn.prepare(query)?;
        let entities = stmt
            .query_map([], |row| {
                let type_str: String = row.get(2)?;
                let created_at_str: String = row.get(6)?;
                Ok(Entity {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    entity_type: type_str.parse().unwrap_or(EntityType::Person),
                    icon: row.get(3)?,
                    color: row.get(4)?,
                    archived: row.get(5)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entities)
    }

    /// List entities by type
    pub fn list_entities_by_type(&self, entity_type: EntityType) -> Result<Vec<Entity>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, type, icon, color, archived, created_at FROM entities WHERE type = ? AND archived = 0 ORDER BY name",
        )?;

        let entities = stmt
            .query_map(params![entity_type.as_str()], |row| {
                let type_str: String = row.get(2)?;
                let created_at_str: String = row.get(6)?;
                Ok(Entity {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    entity_type: type_str.parse().unwrap_or(EntityType::Person),
                    icon: row.get(3)?,
                    color: row.get(4)?,
                    archived: row.get(5)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entities)
    }

    /// Update an entity
    pub fn update_entity(
        &self,
        id: i64,
        name: Option<&str>,
        icon: Option<&str>,
        color: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn()?;

        // Use explicit transaction for atomicity when multiple fields updated
        conn.execute("BEGIN TRANSACTION", [])?;

        let result = (|| {
            if let Some(name) = name {
                conn.execute(
                    "UPDATE entities SET name = ? WHERE id = ?",
                    params![name, id],
                )?;
            }
            if let Some(icon) = icon {
                conn.execute(
                    "UPDATE entities SET icon = ? WHERE id = ?",
                    params![icon, id],
                )?;
            }
            if let Some(color) = color {
                conn.execute(
                    "UPDATE entities SET color = ? WHERE id = ?",
                    params![color, id],
                )?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    /// Archive an entity (soft delete)
    pub fn archive_entity(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("UPDATE entities SET archived = 1 WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Unarchive an entity
    pub fn unarchive_entity(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("UPDATE entities SET archived = 0 WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Delete an entity permanently (use with caution - prefer archive)
    pub fn delete_entity(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM entities WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Count splits associated with an entity
    pub fn count_splits_by_entity(&self, entity_id: i64) -> Result<i64> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transaction_splits WHERE entity_id = ?",
            params![entity_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // ========== Location Operations ==========

    /// Create a new location
    pub fn create_location(&self, location: &NewLocation) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO locations (name, address, city, state, country, latitude, longitude, location_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                location.name,
                location.address,
                location.city,
                location.state,
                location.country.as_deref().unwrap_or("US"),
                location.latitude,
                location.longitude,
                location.location_type.map(|t| t.as_str())
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get a location by ID
    pub fn get_location(&self, id: i64) -> Result<Option<Location>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, name, address, city, state, country, latitude, longitude, location_type, created_at FROM locations WHERE id = ?",
            params![id],
            |row| {
                let type_str: Option<String> = row.get(8)?;
                let created_at_str: String = row.get(9)?;
                Ok(Location {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    address: row.get(2)?,
                    city: row.get(3)?,
                    state: row.get(4)?,
                    country: row.get::<_, Option<String>>(5)?.unwrap_or_else(|| "US".to_string()),
                    latitude: row.get(6)?,
                    longitude: row.get(7)?,
                    location_type: type_str.and_then(|s| s.parse().ok()),
                    created_at: parse_datetime(&created_at_str),
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    /// List all locations
    pub fn list_locations(&self) -> Result<Vec<Location>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, address, city, state, country, latitude, longitude, location_type, created_at FROM locations ORDER BY name",
        )?;

        let locations = stmt
            .query_map([], |row| {
                let type_str: Option<String> = row.get(8)?;
                let created_at_str: String = row.get(9)?;
                Ok(Location {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    address: row.get(2)?,
                    city: row.get(3)?,
                    state: row.get(4)?,
                    country: row
                        .get::<_, Option<String>>(5)?
                        .unwrap_or_else(|| "US".to_string()),
                    latitude: row.get(6)?,
                    longitude: row.get(7)?,
                    location_type: type_str.and_then(|s| s.parse().ok()),
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(locations)
    }

    /// Delete a location
    pub fn delete_location(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM locations WHERE id = ?", params![id])?;
        Ok(())
    }

    // ========== Transaction Split Operations ==========

    /// Create a new split for a transaction
    pub fn create_split(&self, split: &NewTransactionSplit) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO transaction_splits (transaction_id, amount, description, split_type, entity_id, purchaser_id) VALUES (?, ?, ?, ?, ?, ?)",
            params![
                split.transaction_id,
                split.amount,
                split.description,
                split.split_type.as_str(),
                split.entity_id,
                split.purchaser_id
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get splits for a transaction
    pub fn get_splits_for_transaction(&self, transaction_id: i64) -> Result<Vec<TransactionSplit>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, transaction_id, amount, description, split_type, entity_id, purchaser_id, created_at
             FROM transaction_splits WHERE transaction_id = ? ORDER BY id",
        )?;

        let splits = stmt
            .query_map(params![transaction_id], |row| {
                let type_str: String = row.get(4)?;
                let created_at_str: String = row.get(7)?;
                Ok(TransactionSplit {
                    id: row.get(0)?,
                    transaction_id: row.get(1)?,
                    amount: row.get(2)?,
                    description: row.get(3)?,
                    split_type: type_str.parse().unwrap_or(SplitType::Item),
                    entity_id: row.get(5)?,
                    purchaser_id: row.get(6)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(splits)
    }

    /// Get splits with entity details for a transaction
    pub fn get_splits_with_details(
        &self,
        transaction_id: i64,
    ) -> Result<Vec<TransactionSplitWithDetails>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT s.id, s.transaction_id, s.amount, s.description, s.split_type,
                    s.entity_id, s.purchaser_id, s.created_at,
                    e.name as entity_name, p.name as purchaser_name
             FROM transaction_splits s
             LEFT JOIN entities e ON s.entity_id = e.id
             LEFT JOIN entities p ON s.purchaser_id = p.id
             WHERE s.transaction_id = ? ORDER BY s.id",
        )?;

        let splits = stmt
            .query_map(params![transaction_id], |row| {
                let type_str: String = row.get(4)?;
                let created_at_str: String = row.get(7)?;
                Ok(TransactionSplitWithDetails {
                    split: TransactionSplit {
                        id: row.get(0)?,
                        transaction_id: row.get(1)?,
                        amount: row.get(2)?,
                        description: row.get(3)?,
                        split_type: type_str.parse().unwrap_or(SplitType::Item),
                        entity_id: row.get(5)?,
                        purchaser_id: row.get(6)?,
                        created_at: parse_datetime(&created_at_str),
                    },
                    entity_name: row.get(8)?,
                    purchaser_name: row.get(9)?,
                    tags: vec![], // Tags loaded separately if needed
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(splits)
    }

    /// Get a single split by ID
    pub fn get_split_by_id(&self, id: i64) -> Result<Option<TransactionSplit>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, transaction_id, amount, description, split_type, entity_id, purchaser_id, created_at
             FROM transaction_splits WHERE id = ?",
        )?;

        let split = stmt
            .query_row(params![id], |row| {
                let type_str: String = row.get(4)?;
                let created_at_str: String = row.get(7)?;
                Ok(TransactionSplit {
                    id: row.get(0)?,
                    transaction_id: row.get(1)?,
                    amount: row.get(2)?,
                    description: row.get(3)?,
                    split_type: type_str.parse().unwrap_or(SplitType::Item),
                    entity_id: row.get(5)?,
                    purchaser_id: row.get(6)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })
            .optional()?;

        Ok(split)
    }

    /// Update a split
    pub fn update_split(
        &self,
        id: i64,
        amount: Option<f64>,
        description: Option<&str>,
        split_type: Option<SplitType>,
        entity_id: Option<Option<i64>>,
        purchaser_id: Option<Option<i64>>,
    ) -> Result<()> {
        let conn = self.conn()?;

        if let Some(amount) = amount {
            conn.execute(
                "UPDATE transaction_splits SET amount = ? WHERE id = ?",
                params![amount, id],
            )?;
        }
        if let Some(description) = description {
            conn.execute(
                "UPDATE transaction_splits SET description = ? WHERE id = ?",
                params![description, id],
            )?;
        }
        if let Some(split_type) = split_type {
            conn.execute(
                "UPDATE transaction_splits SET split_type = ? WHERE id = ?",
                params![split_type.as_str(), id],
            )?;
        }
        if let Some(entity_id) = entity_id {
            conn.execute(
                "UPDATE transaction_splits SET entity_id = ? WHERE id = ?",
                params![entity_id, id],
            )?;
        }
        if let Some(purchaser_id) = purchaser_id {
            conn.execute(
                "UPDATE transaction_splits SET purchaser_id = ? WHERE id = ?",
                params![purchaser_id, id],
            )?;
        }

        Ok(())
    }

    /// Delete a split
    pub fn delete_split(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM transaction_splits WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Delete all splits for a transaction
    pub fn delete_splits_for_transaction(&self, transaction_id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM transaction_splits WHERE transaction_id = ?",
            params![transaction_id],
        )?;
        Ok(())
    }

    /// Add a tag to a split
    pub fn add_split_tag(
        &self,
        split_id: i64,
        tag_id: i64,
        source: TagSource,
        confidence: Option<f64>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO split_tags (split_id, tag_id, source, confidence) VALUES (?, ?, ?, ?)",
            params![split_id, tag_id, source.as_str(), confidence],
        )?;
        Ok(())
    }

    /// Remove a tag from a split
    pub fn remove_split_tag(&self, split_id: i64, tag_id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM split_tags WHERE split_id = ? AND tag_id = ?",
            params![split_id, tag_id],
        )?;
        Ok(())
    }

    // ========== Trip Operations ==========

    /// Create a new trip
    pub fn create_trip(&self, trip: &NewTrip) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO trips (name, description, start_date, end_date, location_id, budget)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                trip.name,
                trip.description,
                trip.start_date.map(|d| d.to_string()),
                trip.end_date.map(|d| d.to_string()),
                trip.location_id,
                trip.budget
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get a trip by ID
    pub fn get_trip(&self, id: i64) -> Result<Option<Trip>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, start_date, end_date, location_id, budget, archived, created_at
             FROM trips WHERE id = ?",
        )?;

        let trip = stmt
            .query_row(params![id], |row| {
                let start_str: Option<String> = row.get(3)?;
                let end_str: Option<String> = row.get(4)?;
                let created_at_str: String = row.get(8)?;
                Ok(Trip {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    start_date: start_str
                        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    end_date: end_str
                        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    location_id: row.get(5)?,
                    budget: row.get(6)?,
                    archived: row.get(7)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })
            .optional()?;

        Ok(trip)
    }

    /// List all trips
    pub fn list_trips(&self, include_archived: bool) -> Result<Vec<Trip>> {
        let conn = self.conn()?;
        let sql = if include_archived {
            "SELECT id, name, description, start_date, end_date, location_id, budget, archived, created_at
             FROM trips ORDER BY start_date DESC NULLS LAST"
        } else {
            "SELECT id, name, description, start_date, end_date, location_id, budget, archived, created_at
             FROM trips WHERE archived = 0 ORDER BY start_date DESC NULLS LAST"
        };

        let mut stmt = conn.prepare(sql)?;
        let trips = stmt
            .query_map([], |row| {
                let start_str: Option<String> = row.get(3)?;
                let end_str: Option<String> = row.get(4)?;
                let created_at_str: String = row.get(8)?;
                Ok(Trip {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    start_date: start_str
                        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    end_date: end_str
                        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    location_id: row.get(5)?,
                    budget: row.get(6)?,
                    archived: row.get(7)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(trips)
    }

    /// Update a trip
    pub fn update_trip(
        &self,
        id: i64,
        name: Option<&str>,
        description: Option<&str>,
        start_date: Option<Option<NaiveDate>>,
        end_date: Option<Option<NaiveDate>>,
        location_id: Option<Option<i64>>,
        budget: Option<Option<f64>>,
    ) -> Result<()> {
        let conn = self.conn()?;

        if let Some(name) = name {
            conn.execute("UPDATE trips SET name = ? WHERE id = ?", params![name, id])?;
        }
        if let Some(description) = description {
            conn.execute(
                "UPDATE trips SET description = ? WHERE id = ?",
                params![description, id],
            )?;
        }
        if let Some(start_date) = start_date {
            conn.execute(
                "UPDATE trips SET start_date = ? WHERE id = ?",
                params![start_date.map(|d| d.to_string()), id],
            )?;
        }
        if let Some(end_date) = end_date {
            conn.execute(
                "UPDATE trips SET end_date = ? WHERE id = ?",
                params![end_date.map(|d| d.to_string()), id],
            )?;
        }
        if let Some(location_id) = location_id {
            conn.execute(
                "UPDATE trips SET location_id = ? WHERE id = ?",
                params![location_id, id],
            )?;
        }
        if let Some(budget) = budget {
            conn.execute(
                "UPDATE trips SET budget = ? WHERE id = ?",
                params![budget, id],
            )?;
        }

        Ok(())
    }

    /// Archive a trip
    pub fn archive_trip(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("UPDATE trips SET archived = 1 WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Delete a trip
    pub fn delete_trip(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;

        // Use explicit transaction for atomicity
        conn.execute("BEGIN TRANSACTION", [])?;

        let result = (|| {
            // Unlink transactions from trip first
            conn.execute(
                "UPDATE transactions SET trip_id = NULL WHERE trip_id = ?",
                params![id],
            )?;
            conn.execute("DELETE FROM trips WHERE id = ?", params![id])?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    /// Assign transaction to a trip
    pub fn assign_transaction_to_trip(
        &self,
        transaction_id: i64,
        trip_id: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE transactions SET trip_id = ? WHERE id = ?",
            params![trip_id, transaction_id],
        )?;
        Ok(())
    }

    /// Get transactions for a trip
    pub fn get_trip_transactions(&self, trip_id: i64) -> Result<Vec<Transaction>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, date, description, amount, category, merchant_normalized,
                    import_hash, purchase_location_id, vendor_location_id, trip_id,
                    source, expected_amount, archived, original_data, import_format, card_member, payment_method, created_at
             FROM transactions WHERE trip_id = ? AND archived = 0 ORDER BY date DESC",
        )?;

        let transactions = stmt
            .query_map(params![trip_id], |row| Self::row_to_transaction(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(transactions)
    }

    /// Get trip spending total
    pub fn get_trip_spending(&self, trip_id: i64) -> Result<(f64, i64)> {
        let conn = self.conn()?;
        let (total, count): (f64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(ABS(amount)), 0), COUNT(*) FROM transactions WHERE trip_id = ? AND amount < 0",
            params![trip_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok((total, count))
    }

    // ========== Mileage Log Operations ==========

    /// Create a mileage log entry
    pub fn create_mileage_log(&self, log: &NewMileageLog) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO mileage_logs (entity_id, date, odometer, note) VALUES (?, ?, ?, ?)",
            params![log.entity_id, log.date.to_string(), log.odometer, log.note],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get mileage logs for an entity
    pub fn get_mileage_logs(&self, entity_id: i64) -> Result<Vec<MileageLog>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, entity_id, date, odometer, note, created_at
             FROM mileage_logs WHERE entity_id = ? ORDER BY date DESC",
        )?;

        let logs = stmt
            .query_map(params![entity_id], |row| {
                let date_str: String = row.get(2)?;
                let created_at_str: String = row.get(5)?;
                Ok(MileageLog {
                    id: row.get(0)?,
                    entity_id: row.get(1)?,
                    date: chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_default(),
                    odometer: row.get(3)?,
                    note: row.get(4)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(logs)
    }

    /// Get a single mileage log entry by ID
    pub fn get_mileage_log(&self, id: i64) -> Result<Option<MileageLog>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, entity_id, date, odometer, note, created_at
             FROM mileage_logs WHERE id = ?",
        )?;

        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(row) => {
                let date_str: String = row.get(2)?;
                let created_at_str: String = row.get(5)?;
                Ok(Some(MileageLog {
                    id: row.get(0)?,
                    entity_id: row.get(1)?,
                    date: chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_default(),
                    odometer: row.get(3)?,
                    note: row.get(4)?,
                    created_at: parse_datetime(&created_at_str),
                }))
            }
            None => Ok(None),
        }
    }

    /// Delete a mileage log entry
    pub fn delete_mileage_log(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM mileage_logs WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Get total miles for a vehicle (difference between first and last odometer readings)
    pub fn get_vehicle_total_miles(&self, entity_id: i64) -> Result<Option<f64>> {
        let conn = self.conn()?;
        let result: (Option<f64>, Option<f64>) = conn.query_row(
            "SELECT MIN(odometer), MAX(odometer) FROM mileage_logs WHERE entity_id = ?",
            params![entity_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        match result {
            (Some(min), Some(max)) if max > min => Ok(Some(max - min)),
            _ => Ok(None),
        }
    }

    // ========== Location Spending Reports ==========

    /// Get spending by location
    pub fn get_spending_by_location(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<LocationSpending>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT l.id, l.name, l.city, l.country,
                    COALESCE(SUM(ABS(t.amount)), 0) as total, COUNT(t.id) as count
             FROM locations l
             LEFT JOIN transactions t ON t.purchase_location_id = l.id
                  AND t.date BETWEEN ? AND ? AND t.amount < 0
             GROUP BY l.id
             HAVING count > 0
             ORDER BY total DESC",
        )?;

        let results = stmt
            .query_map(params![from.to_string(), to.to_string()], |row| {
                Ok(LocationSpending {
                    location_id: row.get(0)?,
                    location_name: row.get(1)?,
                    city: row.get(2)?,
                    country: row.get(3)?,
                    total_spent: row.get(4)?,
                    transaction_count: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Update transaction location
    pub fn update_transaction_location(
        &self,
        transaction_id: i64,
        purchase_location_id: Option<i64>,
        vendor_location_id: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE transactions SET purchase_location_id = ?, vendor_location_id = ? WHERE id = ?",
            params![purchase_location_id, vendor_location_id, transaction_id],
        )?;
        Ok(())
    }

    /// Get spending by entity
    pub fn get_spending_by_entity(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<(Entity, f64, i64)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT e.id, e.name, e.type, e.icon, e.color, e.archived, e.created_at,
                    COALESCE(SUM(ABS(s.amount)), 0) as total, COUNT(s.id) as count
             FROM entities e
             LEFT JOIN transaction_splits s ON s.entity_id = e.id
             LEFT JOIN transactions t ON s.transaction_id = t.id AND t.date BETWEEN ? AND ?
             WHERE e.archived = 0
             GROUP BY e.id
             ORDER BY total DESC",
        )?;

        let results = stmt
            .query_map(params![from.to_string(), to.to_string()], |row| {
                let type_str: String = row.get(2)?;
                let created_at_str: String = row.get(6)?;
                Ok((
                    Entity {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        entity_type: type_str.parse().unwrap_or(EntityType::Person),
                        icon: row.get(3)?,
                        color: row.get(4)?,
                        archived: row.get(5)?,
                        created_at: parse_datetime(&created_at_str),
                    },
                    row.get::<_, f64>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    // ========== Vehicle & Property Reports ==========

    /// Get vehicle cost summary (fuel, maintenance, insurance, etc.)
    pub fn get_vehicle_cost_summary(
        &self,
        entity_id: i64,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<VehicleCostSummary> {
        let conn = self.conn()?;

        // Get entity name
        let entity = self
            .get_entity(entity_id)?
            .ok_or_else(|| Error::NotFound(format!("Entity {} not found", entity_id)))?;

        // Get total spending by tag for this vehicle entity
        let mut stmt = conn.prepare(
            "SELECT t.name, COALESCE(SUM(ABS(s.amount)), 0) as total
             FROM transaction_splits s
             LEFT JOIN split_tags st ON st.split_id = s.id
             LEFT JOIN tags t ON st.tag_id = t.id
             LEFT JOIN transactions tx ON s.transaction_id = tx.id
             WHERE s.entity_id = ? AND tx.date BETWEEN ? AND ?
             GROUP BY t.id",
        )?;

        let mut fuel_cost = 0.0;
        let mut maintenance_cost = 0.0;
        let mut insurance_cost = 0.0;
        let mut other_cost = 0.0;

        let rows = stmt.query_map(
            params![entity_id, from.to_string(), to.to_string()],
            |row| {
                let tag_name: Option<String> = row.get(0)?;
                let amount: f64 = row.get(1)?;
                Ok((tag_name, amount))
            },
        )?;

        for row in rows {
            let (tag_name, amount) = row?;
            match tag_name.as_deref().map(|s| s.to_lowercase()) {
                Some(s) if s.contains("fuel") || s.contains("gas") => fuel_cost += amount,
                Some(s)
                    if s.contains("maintenance")
                        || s.contains("repair")
                        || s.contains("service") =>
                {
                    maintenance_cost += amount
                }
                Some(s) if s.contains("insurance") => insurance_cost += amount,
                _ => other_cost += amount,
            }
        }

        let total_cost = fuel_cost + maintenance_cost + insurance_cost + other_cost;

        // Get total miles driven in period
        let total_miles = self.get_vehicle_total_miles(entity_id)?;

        // Calculate cost per mile (if we have miles)
        let cost_per_mile = total_miles.filter(|m| *m > 0.0).map(|m| total_cost / m);

        Ok(VehicleCostSummary {
            entity_id,
            entity_name: entity.name,
            total_cost,
            fuel_cost,
            maintenance_cost,
            insurance_cost,
            other_cost,
            total_miles,
            cost_per_mile,
        })
    }

    /// Get property expense summary (repairs, utilities, furnishings, taxes, etc.)
    pub fn get_property_expense_summary(
        &self,
        entity_id: i64,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<PropertyExpenseSummary> {
        let conn = self.conn()?;

        // Get entity name
        let entity = self
            .get_entity(entity_id)?
            .ok_or_else(|| Error::NotFound(format!("Entity {} not found", entity_id)))?;

        // Get total spending by tag for this property entity
        let mut stmt = conn.prepare(
            "SELECT t.name, COALESCE(SUM(ABS(s.amount)), 0) as total
             FROM transaction_splits s
             LEFT JOIN split_tags st ON st.split_id = s.id
             LEFT JOIN tags t ON st.tag_id = t.id
             LEFT JOIN transactions tx ON s.transaction_id = tx.id
             WHERE s.entity_id = ? AND tx.date BETWEEN ? AND ?
             GROUP BY t.id",
        )?;

        let mut mortgage_rent = 0.0;
        let mut utilities = 0.0;
        let mut maintenance = 0.0;
        let mut taxes = 0.0;
        let mut insurance = 0.0;
        let mut improvements = 0.0;
        let mut other = 0.0;

        let rows = stmt.query_map(
            params![entity_id, from.to_string(), to.to_string()],
            |row| {
                let tag_name: Option<String> = row.get(0)?;
                let amount: f64 = row.get(1)?;
                Ok((tag_name, amount))
            },
        )?;

        for row in rows {
            let (tag_name, amount) = row?;
            match tag_name.as_deref().map(|s| s.to_lowercase()) {
                Some(s) if s.contains("mortgage") || s.contains("rent") || s.contains("loan") => {
                    mortgage_rent += amount
                }
                Some(s)
                    if s.contains("utilit")
                        || s.contains("electric")
                        || s.contains("water")
                        || s.contains("gas") =>
                {
                    utilities += amount
                }
                Some(s)
                    if s.contains("repair") || s.contains("maintenance") || s.contains("fix") =>
                {
                    maintenance += amount
                }
                Some(s) if s.contains("tax") || s.contains("property tax") => taxes += amount,
                Some(s) if s.contains("insurance") || s.contains("homeowner") => {
                    insurance += amount
                }
                Some(s)
                    if s.contains("improve")
                        || s.contains("renovation")
                        || s.contains("upgrade") =>
                {
                    improvements += amount
                }
                _ => other += amount,
            }
        }

        let total_expenses =
            mortgage_rent + utilities + maintenance + taxes + insurance + improvements + other;

        Ok(PropertyExpenseSummary {
            entity_id,
            entity_name: entity.name,
            total_expenses,
            mortgage_rent,
            utilities,
            maintenance,
            taxes,
            insurance,
            improvements,
            other,
        })
    }
}
