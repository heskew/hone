//! CSV import parsers for various bank formats

use chrono::NaiveDate;
use csv::{ReaderBuilder, StringRecord};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use tracing::debug;

use crate::error::{Error, Result};
use crate::models::{Bank, NewTransaction, PaymentMethod};

/// Convert a CSV record to a JSON object using headers as keys
fn record_to_json(headers: &StringRecord, record: &StringRecord) -> String {
    let mut map = serde_json::Map::new();
    for (i, header) in headers.iter().enumerate() {
        if let Some(value) = record.get(i) {
            map.insert(header.to_string(), Value::String(value.to_string()));
        }
    }
    json!(map).to_string()
}

/// Parse CSV data from a bank into transactions
pub fn parse_csv<R: Read>(reader: R, bank: Bank) -> Result<Vec<NewTransaction>> {
    match bank {
        Bank::Chase => parse_chase(reader),
        Bank::Bofa => parse_bofa(reader),
        Bank::Amex => parse_amex(reader),
        Bank::CapitalOne => parse_capitalone(reader),
    }
}

/// Detect bank format from CSV header line
///
/// Returns None if the format is not recognized.
pub fn detect_bank_format(header: &str) -> Option<Bank> {
    let header = header.trim();

    // Capital One: "Transaction Date,Posted Date,Card No.,..."
    // Note: "Posted" with 'ed' distinguishes from Chase's "Post Date"
    if header.starts_with("Transaction Date,Posted Date,Card No.") {
        return Some(Bank::CapitalOne);
    }

    // Chase: "Transaction Date,Post Date,Description,Category,Type,Amount,..."
    if header.starts_with("Transaction Date,Post Date,Description,Category,Type,Amount") {
        return Some(Bank::Chase);
    }

    // Amex extended format: "Date,Description,Card Member,Account #,Amount,..."
    // Has 13 columns including Extended Details, Category, etc.
    if header.starts_with("Date,Description,Card Member,Account #,Amount") {
        return Some(Bank::Amex);
    }

    // BofA and Amex simple both start with "Date,Description,Amount"
    // BofA has a 4th column (Running Bal. or Balance), Amex simple has only 3
    if header.starts_with("Date,Description,Amount") {
        // Check for 4th column indicators
        if header.contains("Running") || header.contains("Balance") {
            return Some(Bank::Bofa);
        }
        // Count columns - if only 3, it's Amex simple format
        let column_count = header.split(',').count();
        if column_count == 3 {
            return Some(Bank::Amex);
        }
        // 4+ columns without Running/Balance - assume BofA-like format
        return Some(Bank::Bofa);
    }

    None
}

/// Generate a unique hash for deduplication
fn generate_hash(date: &NaiveDate, description: &str, amount: f64) -> String {
    generate_hash_with_ref(date, description, amount, None)
}

/// Generate hash with optional reference number for banks that provide unique transaction IDs
fn generate_hash_with_ref(
    date: &NaiveDate,
    description: &str,
    amount: f64,
    reference: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(date.to_string().as_bytes());
    hasher.update(description.as_bytes());
    hasher.update(amount.to_be_bytes());
    // Include reference number if available (e.g., Amex extended format)
    // This distinguishes separate transactions with identical date/description/amount
    if let Some(ref_str) = reference {
        hasher.update(ref_str.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Parse Chase CSV format
/// Format: Transaction Date,Post Date,Description,Category,Type,Amount,Memo
fn parse_chase<R: Read>(reader: R) -> Result<Vec<NewTransaction>> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(reader);

    let headers = rdr.headers()?.clone();
    let mut transactions = Vec::new();

    for result in rdr.records() {
        let record = result?;

        // Capture original data as JSON
        let original_data = Some(record_to_json(&headers, &record));

        // Transaction Date is column 0
        let date_str = record
            .get(0)
            .ok_or_else(|| Error::Import("Missing date".into()))?;
        let date = parse_date(date_str)?;

        // Description is column 2
        let description = record
            .get(2)
            .ok_or_else(|| Error::Import("Missing description".into()))?
            .to_string();

        // Category is column 3
        let category = record
            .get(3)
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());

        // Amount is column 5 (negative = expense, positive = credit)
        let amount_str = record
            .get(5)
            .ok_or_else(|| Error::Import("Missing amount".into()))?;
        let amount = parse_amount(amount_str)?;

        let import_hash = generate_hash(&date, &description, amount);

        transactions.push(NewTransaction {
            date,
            description,
            amount,
            category,
            import_hash,
            original_data,
            import_format: Some("chase_csv".to_string()),
            card_member: None,
            payment_method: None,
        });
    }

    debug!("Parsed {} Chase transactions", transactions.len());
    Ok(transactions)
}

/// Parse Bank of America CSV format
/// Format: Date,Description,Amount,Running Bal.
fn parse_bofa<R: Read>(reader: R) -> Result<Vec<NewTransaction>> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(reader);

    let headers = rdr.headers()?.clone();
    let mut transactions = Vec::new();

    for result in rdr.records() {
        let record = result?;

        // Capture original data as JSON
        let original_data = Some(record_to_json(&headers, &record));

        let date_str = record
            .get(0)
            .ok_or_else(|| Error::Import("Missing date".into()))?;
        let date = parse_date(date_str)?;

        let description = record
            .get(1)
            .ok_or_else(|| Error::Import("Missing description".into()))?
            .to_string();

        let amount_str = record
            .get(2)
            .ok_or_else(|| Error::Import("Missing amount".into()))?;
        let amount = parse_amount(amount_str)?;

        let import_hash = generate_hash(&date, &description, amount);

        transactions.push(NewTransaction {
            date,
            description,
            amount,
            category: None,
            import_hash,
            original_data,
            import_format: Some("bofa_csv".to_string()),
            card_member: None,
            payment_method: None,
        });
    }

    debug!("Parsed {} BofA transactions", transactions.len());
    Ok(transactions)
}

/// Parse American Express CSV format
/// Simple format: Date,Description,Amount (3 columns)
/// Extended format: Date,Description,Card Member,Account #,Amount,Extended Details,
///                  Appears On Your Statement As,Address,City/State,Zip Code,Country,Reference,Category
/// Note: Amex shows expenses as POSITIVE numbers (inverted from typical)
fn parse_amex<R: Read>(reader: R) -> Result<Vec<NewTransaction>> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(reader);

    // Detect format from headers
    let headers = rdr.headers()?.clone();
    let is_extended = headers.len() > 3 && headers.get(2) == Some("Card Member");

    let (amount_col, category_col): (usize, Option<usize>) = if is_extended {
        // Extended format: Amount is column 4, Category is column 12
        (4, Some(12))
    } else {
        // Simple format: Amount is column 2, no category
        (2, None)
    };

    debug!(
        "Parsing Amex CSV with {} format ({} columns)",
        if is_extended { "extended" } else { "simple" },
        headers.len()
    );

    let mut transactions = Vec::new();

    for result in rdr.records() {
        let record = result?;

        // Capture original data as JSON
        let original_data = Some(record_to_json(&headers, &record));

        let date_str = record
            .get(0)
            .ok_or_else(|| Error::Import("Missing date".into()))?;
        let date = parse_date(date_str)?;

        let raw_description = record
            .get(1)
            .ok_or_else(|| Error::Import("Missing description".into()))?
            .to_string();

        // For extended format, try multiple sources for the best merchant name:
        // 1. Extended Details (col 5) sometimes has cleaner merchant info embedded
        // 2. "Appears On Your Statement As" (col 6) is usually good but sometimes truncated
        // 3. Description (col 1) is fallback
        let mut description = if is_extended {
            let appears_on_statement = record.get(6).map(|s| s.trim()).filter(|s| s.len() > 3);

            let extended_details = record.get(5).map(|s| s.trim());

            // Try to extract better merchant name from Extended Details
            // It often contains patterns like "AplPay MERCHANT*NAME" or "Description : MERCHANT"
            let from_extended = extended_details
                .and_then(|details| extract_merchant_from_extended_details(details));

            // Use extracted merchant from Extended Details if available
            // Extended Details often has the full merchant name when Description is truncated
            if let Some(extracted) = from_extended {
                extracted
            } else {
                appears_on_statement
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| raw_description.clone())
            }
        } else {
            raw_description.clone()
        };

        // Detect payment method from description prefix (e.g., "AplPay HAPPY LEMON")
        let payment_method = if description.starts_with("AplPay ") {
            // Strip the prefix from the description
            description = description[7..].to_string();
            Some(PaymentMethod::ApplePay)
        } else if description.starts_with("APPLE PAY ") {
            description = description[10..].to_string();
            Some(PaymentMethod::ApplePay)
        } else if description.starts_with("GOOGLE PAY ") || description.starts_with("GPay ") {
            let prefix_len = if description.starts_with("GOOGLE PAY ") {
                11
            } else {
                5
            };
            description = description[prefix_len..].to_string();
            Some(PaymentMethod::GooglePay)
        } else {
            None
        };

        let amount_str = record
            .get(amount_col)
            .ok_or_else(|| Error::Import("Missing amount".into()))?;
        // Invert Amex amounts: positive charges become negative expenses
        let amount = -parse_amount(amount_str)?;

        // Extract category from extended format if available
        let category = category_col
            .and_then(|col| record.get(col))
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());

        // Extract card member from extended format (column 2)
        let card_member = if is_extended {
            record
                .get(2)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        } else {
            None
        };

        // Extract reference number from extended format (column 11)
        // This distinguishes separate transactions with identical date/description/amount
        let reference = if is_extended {
            record
                .get(11)
                .map(|s| s.trim().trim_matches('\''))
                .filter(|s| !s.is_empty())
        } else {
            None
        };

        // Use raw_description for hash; include reference for extended format
        let import_hash = generate_hash_with_ref(&date, &raw_description, amount, reference);

        transactions.push(NewTransaction {
            date,
            description,
            amount,
            category,
            import_hash,
            original_data,
            import_format: Some("amex_csv".to_string()),
            card_member,
            payment_method,
        });
    }

    debug!("Parsed {} Amex transactions", transactions.len());
    Ok(transactions)
}

/// Parse Capital One CSV format
/// Format: Transaction Date,Posted Date,Card No.,Description,Category,Debit,Credit
fn parse_capitalone<R: Read>(reader: R) -> Result<Vec<NewTransaction>> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(reader);

    let headers = rdr.headers()?.clone();
    let mut transactions = Vec::new();

    for result in rdr.records() {
        let record = result?;

        // Capture original data as JSON
        let original_data = Some(record_to_json(&headers, &record));

        let date_str = record
            .get(0)
            .ok_or_else(|| Error::Import("Missing date".into()))?;
        let date = parse_date(date_str)?;

        let description = record
            .get(3)
            .ok_or_else(|| Error::Import("Missing description".into()))?
            .to_string();

        let category = record
            .get(4)
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());

        // Debit is column 5, Credit is column 6
        let debit_str = record.get(5).unwrap_or("");
        let credit_str = record.get(6).unwrap_or("");

        let amount = if !debit_str.is_empty() {
            -parse_amount(debit_str)? // Debits are expenses (negative)
        } else if !credit_str.is_empty() {
            parse_amount(credit_str)? // Credits are income (positive)
        } else {
            continue; // Skip rows with no amount
        };

        let import_hash = generate_hash(&date, &description, amount);

        transactions.push(NewTransaction {
            date,
            description,
            amount,
            category,
            import_hash,
            original_data,
            import_format: Some("capitalone_csv".to_string()),
            card_member: None,
            payment_method: None,
        });
    }

    debug!("Parsed {} Capital One transactions", transactions.len());
    Ok(transactions)
}

/// Extract merchant name from Amex Extended Details field
///
/// Extended Details often contains better merchant info than the truncated Description field.
/// Pattern: "... AplPay MERCHANTNAME LOCATION ..." where location ends with 2-letter code
///
/// Strategy: Find "AplPay ", then work backwards from end to find location pattern,
/// take everything between AplPay and location as merchant name.
fn extract_merchant_from_extended_details(details: &str) -> Option<String> {
    let details = details.trim();
    if details.is_empty() {
        return None;
    }

    // Look for "AplPay " pattern - merchant name follows it
    let aplpay_pos = details.find("AplPay ")?;
    let after_aplpay = &details[aplpay_pos + 7..]; // Skip "AplPay "

    // Handle newline - take everything before it
    let segment = if let Some(newline_pos) = after_aplpay.find('\n') {
        &after_aplpay[..newline_pos]
    } else {
        after_aplpay
    };

    // Split into words and work backwards to find location
    let words: Vec<&str> = segment.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }

    // Find where location starts by looking for 2-letter country/state code from the end
    // Skip trailing junk (phone numbers starting with +)
    let mut code_idx = None;
    for i in (0..words.len()).rev() {
        let word = words[i];
        // Skip phone numbers and other trailing data
        if word.starts_with('+') || word.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        // Found a 2-letter uppercase code (state/country)
        if word.len() == 2 && word.chars().all(|c| c.is_ascii_uppercase()) {
            code_idx = Some(i);
            break;
        }
    }

    // If no location code found, return the whole thing (minus trailing phone numbers)
    let code_idx = match code_idx {
        Some(idx) => idx,
        None => {
            // Filter out trailing phone numbers
            let end = words
                .iter()
                .position(|w| {
                    w.starts_with('+') || (w.len() > 5 && w.chars().all(|c| c.is_ascii_digit()))
                })
                .unwrap_or(words.len());
            let merchant = words[..end].join(" ");
            return if merchant.len() > 3 {
                Some(merchant)
            } else {
                None
            };
        }
    };

    // Location is: city word(s) + code
    // City could be 1-2 words before the code (e.g., "SEATTLE WA" or "HONG KONG HK")
    // Detect multi-word cities by checking if prev word is also all-caps location-like
    let mut city_start = code_idx;
    if code_idx >= 1 {
        city_start = code_idx - 1;
        // Check for two-word city (HONG KONG, NEW YORK, etc.)
        if city_start >= 1 {
            let prev_word = words[city_start - 1];
            let city_word = words[city_start];
            // If both are uppercase and city_word is a common second part of city names
            if prev_word.chars().all(|c| c.is_ascii_uppercase())
                && (city_word == "KONG"
                    || city_word == "YORK"
                    || city_word == "ANGELES"
                    || city_word == "FRANCISCO"
                    || city_word == "DIEGO"
                    || city_word == "JOSE")
            {
                city_start -= 1;
            }
        }
    }

    if city_start == 0 {
        return None; // No room for merchant
    }

    let merchant = words[..city_start].join(" ");
    if merchant.len() > 3 {
        Some(merchant)
    } else {
        None
    }
}

/// Parse a date string in various common formats
fn parse_date(s: &str) -> Result<NaiveDate> {
    let s = s.trim();

    // Try common date formats
    let formats = [
        "%m/%d/%Y", // 01/15/2024
        "%m/%d/%y", // 01/15/24
        "%Y-%m-%d", // 2024-01-15
        "%m-%d-%Y", // 01-15-2024
        "%d/%m/%Y", // 15/01/2024 (European)
    ];

    for fmt in formats {
        if let Ok(date) = NaiveDate::parse_from_str(s, fmt) {
            return Ok(date);
        }
    }

    Err(Error::Import(format!("Unable to parse date: {}", s)))
}

/// Parse an amount string, handling currency symbols and commas
fn parse_amount(s: &str) -> Result<f64> {
    let cleaned: String = s
        .trim()
        .replace(['$', ',', ' '], "")
        .replace('(', "-")
        .replace(')', "");

    cleaned
        .parse::<f64>()
        .map_err(|_| Error::Import(format!("Unable to parse amount: {}", s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date() {
        assert_eq!(
            parse_date("01/15/2024").unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()
        );
        assert_eq!(
            parse_date("2024-01-15").unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()
        );
    }

    #[test]
    fn test_parse_amount() {
        assert_eq!(parse_amount("$1,234.56").unwrap(), 1234.56);
        assert_eq!(parse_amount("-123.45").unwrap(), -123.45);
        assert_eq!(parse_amount("(100.00)").unwrap(), -100.00);
    }

    #[test]
    fn test_parse_chase() {
        let csv = r#"Transaction Date,Post Date,Description,Category,Type,Amount,Memo
01/15/2024,01/16/2024,NETFLIX.COM,Entertainment,Sale,-15.99,
01/14/2024,01/15/2024,STARBUCKS,Food & Drink,Sale,-5.50,"#;

        let transactions = parse_chase(csv.as_bytes()).unwrap();
        assert_eq!(transactions.len(), 2);
        assert_eq!(transactions[0].description, "NETFLIX.COM");
        assert_eq!(transactions[0].amount, -15.99);
        assert_eq!(transactions[0].category, Some("Entertainment".to_string()));
    }

    #[test]
    fn test_parse_amex() {
        let csv = r#"Date,Description,Amount
01/15/2024,AMAZON.COM,99.99
01/14/2024,REFUND,-25.00"#;

        let transactions = parse_amex(csv.as_bytes()).unwrap();
        assert_eq!(transactions.len(), 2);
        // Amex inverts: positive charges become negative
        assert_eq!(transactions[0].amount, -99.99);
        // Refunds become positive
        assert_eq!(transactions[1].amount, 25.00);
    }

    #[test]
    fn test_parse_amex_extended() {
        // Extended format with 13 columns - Amount is column 4, Category is column 12
        // We use "Appears On Your Statement As" (col 6) for merchant name since it's cleaner
        // than Description (which may have garbage) or Extended Details (which may have addresses)
        let csv = r#"Date,Description,Card Member,Account #,Amount,Extended Details,Appears On Your Statement As,Address,City/State,Zip Code,Country,Reference,Category
01/07/25,ACCOUNTING@ADOBESYS,JOHN DOE,-12345,22.99,"542443008   ADOBE.LY/ENUS
ADOBE Adobe Systems
SAN JOSE
CA",ADOBE ACROPRO SUBS,"500 ADOBE LN","SAN JOSE, CA",95110,UNITED STATES,123456789,Merchandise & Supplies-Internet Purchase
01/06/25,H-E-B #123,JANE DOE,-12345,87.43,"","H-E-B #123","123 MAIN ST","AUSTIN, TX",78701,UNITED STATES,987654321,Merchandise & Supplies-Groceries"#;

        let transactions = parse_amex(csv.as_bytes()).unwrap();
        assert_eq!(transactions.len(), 2);

        // First transaction - Adobe subscription (uses "Appears On Your Statement As" col 6)
        // Note: Description has garbage "ACCOUNTING@ADOBESYS" but we use statement field
        assert_eq!(transactions[0].description, "ADOBE ACROPRO SUBS");
        assert_eq!(transactions[0].amount, -22.99);
        assert_eq!(
            transactions[0].category,
            Some("Merchandise & Supplies-Internet Purchase".to_string())
        );
        // Card member is extracted from extended format
        assert_eq!(transactions[0].card_member, Some("JOHN DOE".to_string()));

        // Second transaction - H-E-B grocery (statement field matches description)
        assert_eq!(transactions[1].description, "H-E-B #123");
        assert_eq!(transactions[1].amount, -87.43);
        assert_eq!(
            transactions[1].category,
            Some("Merchandise & Supplies-Groceries".to_string())
        );
        // Different card member
        assert_eq!(transactions[1].card_member, Some("JANE DOE".to_string()));
    }

    #[test]
    fn test_parse_amex_extended_with_refund() {
        // Extended format with a refund (negative amount)
        let csv = r#"Date,Description,Card Member,Account #,Amount,Extended Details,Appears On Your Statement As,Address,City/State,Zip Code,Country,Reference,Category
01/08/25,AMAZON REFUND,JOHN DOE,-12345,-50.00,"",AMAZON REFUND,"","","",UNITED STATES,111222333,Merchandise & Supplies-Internet Purchase"#;

        let transactions = parse_amex(csv.as_bytes()).unwrap();
        assert_eq!(transactions.len(), 1);

        // Refund should be positive (credit)
        assert_eq!(transactions[0].description, "AMAZON REFUND");
        assert_eq!(transactions[0].amount, 50.00);
    }

    #[test]
    fn test_detect_chase() {
        let header = "Transaction Date,Post Date,Description,Category,Type,Amount,Memo";
        assert_eq!(detect_bank_format(header), Some(Bank::Chase));
    }

    #[test]
    fn test_detect_bofa() {
        let header = "Date,Description,Amount,Running Bal.";
        assert_eq!(detect_bank_format(header), Some(Bank::Bofa));
    }

    #[test]
    fn test_detect_bofa_balance_variant() {
        // BECU and similar banks use "Balance" instead of "Running Bal."
        let header = "Date,Description,Amount,Balance";
        assert_eq!(detect_bank_format(header), Some(Bank::Bofa));
    }

    #[test]
    fn test_detect_amex() {
        let header = "Date,Description,Amount";
        assert_eq!(detect_bank_format(header), Some(Bank::Amex));
    }

    #[test]
    fn test_detect_amex_extended() {
        let header = "Date,Description,Card Member,Account #,Amount,Extended Details,Appears On Your Statement As,Address,City/State,Zip Code,Country,Reference,Category";
        assert_eq!(detect_bank_format(header), Some(Bank::Amex));
    }

    #[test]
    fn test_detect_capitalone() {
        let header = "Transaction Date,Posted Date,Card No.,Description,Category,Debit,Credit";
        assert_eq!(detect_bank_format(header), Some(Bank::CapitalOne));
    }

    #[test]
    fn test_detect_unknown() {
        let header = "Some,Random,Headers,Here";
        assert_eq!(detect_bank_format(header), None);
    }

    #[test]
    fn test_extract_merchant_from_extended_details() {
        // Test case: battery store with truncated Description
        // Extended Details: "CH_3SKUJCCC +15550001234 AplPay SP BATTERYSTORE ANYTOWN GA +15550001234"
        let details = "CH_3SKUJCCC +15550001234 AplPay SP BATTERYSTORE ANYTOWN GA +15550001234";
        assert_eq!(
            extract_merchant_from_extended_details(details),
            Some("SP BATTERYSTORE".to_string())
        );

        // AplPay with US city
        let details = "AplPay HAPPY LEMON SEATTLE WA";
        assert_eq!(
            extract_merchant_from_extended_details(details),
            Some("HAPPY LEMON".to_string())
        );

        // International: Hong Kong (city + country code, no state)
        let details = "AplPay HACKERGADGETS HONG KONG HK";
        assert_eq!(
            extract_merchant_from_extended_details(details),
            Some("HACKERGADGETS".to_string())
        );

        // Multi-line format
        let details = "AplPay MERCHANT NAME\nCompany Inc\nSEATTLE\nWA";
        assert_eq!(
            extract_merchant_from_extended_details(details),
            Some("MERCHANT NAME".to_string())
        );

        // No AplPay pattern - return None
        let details = "Regular text without AplPay";
        assert_eq!(extract_merchant_from_extended_details(details), None);

        // Empty string
        assert_eq!(extract_merchant_from_extended_details(""), None);
    }

    #[test]
    fn test_parse_amex_extended_uses_extended_details() {
        // Test case where Description is truncated but Extended Details has the full name
        let csv = r#"Date,Description,Card Member,Account #,Amount,Extended Details,Appears On Your Statement As,Address,City/State,Zip Code,Country,Reference,Category
12/31/25,AplPay SP BATTERYSTO ANYTOWN GA,TEST USER,-12345,11.99,"CH_3SKUJCCC +15550001234 AplPay SP BATTERYSTORE ANYTOWN GA +15550001234",AplPay SP BATTERYSTO ANYTOWN GA,"123 TEST ST","ANYTOWN, GA",30000,UNITED STATES,320260010035129399,Merchandise & Supplies-Electronics Stores"#;

        let transactions = parse_amex(csv.as_bytes()).unwrap();
        assert_eq!(transactions.len(), 1);

        // Should extract the full merchant name from Extended Details
        assert_eq!(transactions[0].description, "SP BATTERYSTORE");
        assert_eq!(transactions[0].amount, -11.99);
    }
}
