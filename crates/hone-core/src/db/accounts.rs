//! Account operations

use rusqlite::params;

use super::{parse_datetime, Database};
use crate::error::Result;
use crate::models::{Account, AccountType, Bank};

impl Database {
    /// Create or get an account
    pub fn upsert_account(
        &self,
        name: &str,
        bank: Bank,
        account_type: Option<AccountType>,
    ) -> Result<i64> {
        let conn = self.conn()?;

        // Try to find existing account
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM accounts WHERE name = ? AND bank = ?",
                params![name, bank.as_str()],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            return Ok(id);
        }

        // Create new account
        let account_type_str = account_type.map(|t| match t {
            AccountType::Checking => "checking",
            AccountType::Savings => "savings",
            AccountType::Credit => "credit",
        });

        conn.execute(
            "INSERT INTO accounts (name, bank, account_type) VALUES (?, ?, ?)",
            params![name, bank.as_str(), account_type_str],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// List all accounts
    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, bank, account_type, entity_id, created_at FROM accounts ORDER BY name",
        )?;

        let accounts = stmt
            .query_map([], |row| {
                let bank_str: String = row.get(2)?;
                let account_type_str: Option<String> = row.get(3)?;
                let created_at_str: String = row.get(5)?;

                Ok(Account {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    bank: bank_str.parse().unwrap_or(Bank::Chase),
                    account_type: account_type_str.and_then(|s| match s.as_str() {
                        "checking" => Some(AccountType::Checking),
                        "savings" => Some(AccountType::Savings),
                        "credit" => Some(AccountType::Credit),
                        _ => None,
                    }),
                    entity_id: row.get(4)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(accounts)
    }

    /// Get an account by ID
    pub fn get_account(&self, id: i64) -> Result<Option<Account>> {
        let conn = self.conn()?;
        let account = conn
            .query_row(
                "SELECT id, name, bank, account_type, entity_id, created_at FROM accounts WHERE id = ?",
                params![id],
                |row| {
                    let bank_str: String = row.get(2)?;
                    let account_type_str: Option<String> = row.get(3)?;
                    let created_at_str: String = row.get(5)?;

                    Ok(Account {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        bank: bank_str.parse().unwrap_or(Bank::Chase),
                        account_type: account_type_str.and_then(|s| match s.as_str() {
                            "checking" => Some(AccountType::Checking),
                            "savings" => Some(AccountType::Savings),
                            "credit" => Some(AccountType::Credit),
                            _ => None,
                        }),
                        entity_id: row.get(4)?,
                        created_at: parse_datetime(&created_at_str),
                    })
                },
            )
            .ok();

        Ok(account)
    }

    /// Update account's entity (owner) association
    pub fn update_account_entity(&self, account_id: i64, entity_id: Option<i64>) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE accounts SET entity_id = ? WHERE id = ?",
            params![entity_id, account_id],
        )?;
        Ok(())
    }

    /// Update an account's name and bank
    pub fn update_account(&self, id: i64, name: &str, bank: Bank) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE accounts SET name = ?, bank = ? WHERE id = ?",
            params![name, bank.as_str(), id],
        )?;
        Ok(())
    }

    /// Delete an account and all its transactions
    pub fn delete_account(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;

        // Use explicit transaction for atomicity
        conn.execute("BEGIN TRANSACTION", [])?;

        let result = (|| {
            // Delete transaction-related data first (foreign key constraints)
            conn.execute(
                "DELETE FROM transaction_tags WHERE transaction_id IN (SELECT id FROM transactions WHERE account_id = ?)",
                params![id],
            )?;
            conn.execute(
                "DELETE FROM split_tags WHERE split_id IN (SELECT id FROM transaction_splits WHERE transaction_id IN (SELECT id FROM transactions WHERE account_id = ?))",
                params![id],
            )?;
            conn.execute(
                "DELETE FROM transaction_splits WHERE transaction_id IN (SELECT id FROM transactions WHERE account_id = ?)",
                params![id],
            )?;
            conn.execute("DELETE FROM transactions WHERE account_id = ?", params![id])?;
            conn.execute("DELETE FROM accounts WHERE id = ?", params![id])?;
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

    /// List accounts by entity (owner)
    pub fn list_accounts_by_entity(&self, entity_id: i64) -> Result<Vec<Account>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, bank, account_type, entity_id, created_at FROM accounts WHERE entity_id = ? ORDER BY name",
        )?;

        let accounts = stmt
            .query_map(params![entity_id], |row| {
                let bank_str: String = row.get(2)?;
                let account_type_str: Option<String> = row.get(3)?;
                let created_at_str: String = row.get(5)?;

                Ok(Account {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    bank: bank_str.parse().unwrap_or(Bank::Chase),
                    account_type: account_type_str.and_then(|s| match s.as_str() {
                        "checking" => Some(AccountType::Checking),
                        "savings" => Some(AccountType::Savings),
                        "credit" => Some(AccountType::Credit),
                        _ => None,
                    }),
                    entity_id: row.get(4)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(accounts)
    }
}
