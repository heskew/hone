---
title: Receipt Workflow
description: Receipt capture and matching design
date: 2026-01-24
---

## Overview

Receipts are captured at purchase time but transactions appear in bank exports days/weeks later. This design enables a "receipt-first" workflow where receipts create placeholder transactions that are later matched and reconciled with imported bank data.

## Core Workflow

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  Capture        │     │  CSV Import      │     │  Reconciliation │
│  Receipt        │────▶│  (Later)         │────▶│  Review         │
└─────────────────┘     └──────────────────┘     └─────────────────┘
        │                        │                        │
        ▼                        ▼                        ▼
   Create placeholder      Auto-match by           Resolve conflicts:
   transaction with        amount + date +         - Tip discrepancies
   parsed splits           merchant similarity     - Multiple matches
                                                   - No matches
```

## Database Changes

### Receipt Status Tracking

```sql
-- Receipts can exist before transaction is imported
ALTER TABLE receipts ADD COLUMN status TEXT DEFAULT 'matched';
-- status: matched, pending, manual_review

-- Parsed data for matching (extracted by Ollama)
ALTER TABLE receipts ADD COLUMN receipt_date DATE;
ALTER TABLE receipts ADD COLUMN receipt_total REAL;
ALTER TABLE receipts ADD COLUMN receipt_merchant TEXT;
```

### Placeholder Transactions

```sql
-- Add source tracking to transactions
ALTER TABLE transactions ADD COLUMN source TEXT DEFAULT 'import';
-- source: import, receipt, manual

-- Track expected vs actual amounts (for tip discrepancies)
ALTER TABLE transactions ADD COLUMN expected_amount REAL;
-- NULL for normal transactions
-- Set when receipt total differs from imported amount
```

### Merchant Aliases (Learning)

```sql
-- Learn merchant name variations
CREATE TABLE merchant_aliases (
    id INTEGER PRIMARY KEY,
    receipt_name TEXT NOT NULL,      -- "TARGET T-1234"
    canonical_name TEXT NOT NULL,    -- "TARGET"
    bank TEXT,                       -- which bank uses this format
    confidence REAL DEFAULT 1.0,     -- 1.0 = user confirmed, <1.0 = auto-learned
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(receipt_name, bank)
);
```

### New Alert Type

```sql
-- reconciliation alerts for manual review
-- type: 'reconciliation'
-- message describes the issue
-- metadata JSON stores match candidates, discrepancy details
```

## Matching Algorithm

### On CSV Import

For each new transaction:

```
1. Normalize merchant name
   - Strip store numbers, locations ("TARGET #1234 AUSTIN TX" → "TARGET")
   - Check merchant_aliases table
   - Use Ollama for fuzzy matching if needed

2. Find candidate receipts
   WHERE status = 'pending'
   AND ABS(receipt_total - ABS(transaction.amount)) < tolerance
   AND receipt_date BETWEEN transaction.date - 7 AND transaction.date + 3

3. Score candidates
   - Amount match (exact = 100, within $1 = 90, within 5% = 80)
   - Date proximity (same day = 100, ±1 day = 90, ±3 days = 70)
   - Merchant similarity (Ollama or string distance)

4. Decision
   - Single match with score > 80 → auto-link
   - Multiple matches → create reconciliation alert
   - No match → transaction imports normally
   - Receipt remains pending → can attach later
```

### Tip Discrepancy Handling

When receipt total ≠ transaction amount:

```
1. Calculate difference
   diff = ABS(transaction.amount) - receipt_total

2. If diff looks like a tip (positive, reasonable percentage):
   - Store expected_amount = receipt_total
   - Create split for tip: amount=diff, split_type='tip'
   - Mark as auto-reconciled

3. If diff is suspicious (>30% or negative):
   - Create reconciliation alert
   - Flag for manual review
   - "Receipt shows $47.82 but bank charged $147.82 - please verify"
```

## Receipt Input Methods

### Phase 1: Photo/Screenshot Upload

```
POST /api/receipts
Content-Type: multipart/form-data

file: <image data>
account_id: (optional) which account this will charge
```

Response:
```json
{
  "receipt_id": 123,
  "status": "pending",
  "parsed": {
    "merchant": "Target",
    "date": "2026-01-08",
    "total": 87.43,
    "items": [...]
  },
  "placeholder_transaction_id": 456
}
```

### Phase 2: Email Forwarding (Future)

- Dedicated email address: receipts@hone.example.com
- Parse email for:
  - Attached images → vision model
  - HTML receipt body → text extraction
  - Forwarded order confirmations

### CLI Support

```bash
# Upload receipt image
hone receipt add --file receipt.jpg --account "Chase Credit"

# List pending receipts
hone receipts --status pending

# Manual match
hone receipt match 123 --transaction 456

# Dismiss unmatched receipt
hone receipt dismiss 123 --reason "duplicate"
```

## Reconciliation UI

### Pending Receipts View

```
┌─────────────────────────────────────────────────────────────────┐
│ Pending Receipts (3)                                            │
├─────────────────────────────────────────────────────────────────┤
│ ⏳ Target - $87.43 - Jan 8                                      │
│    Waiting for transaction (uploaded 2 days ago)                │
│    [Match Manually] [Dismiss]                                   │
├─────────────────────────────────────────────────────────────────┤
│ ⚠️ Doordash - $47.82 → $57.82 - Jan 7                           │
│    Tip discrepancy: receipt $47.82, charged $57.82 (+$10 tip?)  │
│    [Confirm Tip] [Review] [Flag Issue]                          │
├─────────────────────────────────────────────────────────────────┤
│ ❓ Amazon - $156.23 - Jan 5                                     │
│    Multiple possible matches:                                   │
│    • AMZN*1234 $156.23 Jan 6                                   │
│    • AMAZON.COM $156.23 Jan 7                                  │
│    [Select Match] [Keep Both] [Dismiss]                         │
└─────────────────────────────────────────────────────────────────┘
```

### Transaction Detail with Receipt

```
┌─────────────────────────────────────────────────────────────────┐
│ TARGET #1234 AUSTIN TX                         -$87.43          │
│ Jan 8, 2026                                    Shopping         │
├─────────────────────────────────────────────────────────────────┤
│ 📷 Receipt attached                            [View] [Replace] │
├─────────────────────────────────────────────────────────────────┤
│ Splits:                                                         │
│   T-shirt (Kids)              Shopping         $25.00           │
│   Groceries                   Groceries        $32.00           │
│   Cleaning supplies           Household        $24.99           │
│   Tax                         -                $5.44            │
│                                        Total:  $87.43 ✓         │
├─────────────────────────────────────────────────────────────────┤
│ [Edit Splits] [Add Entity] [Remove Receipt]                     │
└─────────────────────────────────────────────────────────────────┘
```

## Learning & Feedback

### Merchant Alias Learning

When user manually matches or confirms:
1. Extract merchant name patterns from both receipt and transaction
2. Store in merchant_aliases with confidence based on:
   - User confirmation = 1.0
   - Auto-match accepted = 0.8
   - Multiple confirmations increase confidence

### Match Quality Tracking

```sql
CREATE TABLE match_feedback (
    id INTEGER PRIMARY KEY,
    receipt_id INTEGER REFERENCES receipts(id),
    transaction_id INTEGER REFERENCES transactions(id),
    auto_matched BOOLEAN,           -- was this auto-matched?
    user_confirmed BOOLEAN,         -- did user accept/reject?
    match_score REAL,               -- original algorithm score
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

Use this data to tune matching thresholds over time.

## Duplicate Prevention

### Import Deduplication (Existing)

SHA256 hash of (date, description, amount) prevents reimporting same CSV row.

### Receipt Deduplication

Prevent uploading same receipt twice:

```sql
ALTER TABLE receipts ADD COLUMN content_hash TEXT;
-- SHA256 of image data or parsed (merchant, date, total)
```

On upload:
1. Hash the image content
2. Check for existing receipt with same hash
3. If found, return existing receipt (don't create duplicate)

### Receipt → Transaction Duplicate Check

When creating placeholder transaction from receipt:
- Generate import_hash from receipt data
- If transaction with same hash exists, link receipt instead of creating placeholder

## Implementation Phases

### Phase 1: Foundation ✅
- [x] Schema updates (receipt status, transaction source, expected_amount)
- [x] Receipt upload endpoint (photo/screenshot) with AI parsing
- [x] Receipt status workflow (pending → matched/manual_review/orphaned)
- [x] Content hash deduplication
- [x] Manual receipt-to-transaction linking (CLI + API)
- [x] Receipt CLI commands (add, list, match, status, dismiss)
- [x] Merchant aliases table (schema ready for Phase 2)

### Phase 2: Smart Matching
- [x] Auto-matching receipts to imported transactions (amount + date + merchant)
- [x] Merchant name normalization (via Ollama)
- [x] Ollama-assisted merchant matching (uses normalized merchant names)
- [ ] Tip discrepancy detection
- [ ] Reconciliation alerts

### Phase 3: UI
- [ ] Pending receipts view
- [ ] Manual match interface
- [ ] Tip confirmation flow
- [ ] Transaction detail with receipt

### Phase 4: Learning
- [ ] Merchant alias learning from matches
- [ ] Match feedback collection
- [ ] Confidence-based threshold tuning

## Design Decisions

### 1. Placeholder Cleanup

What happens to placeholder transactions that never match?

**Decision**:
- Placeholder transactions are **included in spending reports** (they represent real spending)
- After **90 days** without a match, they're flagged with status `orphaned`
- Orphaned placeholders appear in a dedicated "Orphaned Receipts" section in the reconciliation UI
- User can:
  - Manually match to a transaction
  - Convert to a manual transaction (confirms the receipt as the source of truth)
  - Dismiss (marks as duplicate or erroneous)
- Rationale: Don't auto-delete user data; surface it for decision-making

### 2. Split Preservation

When matching receipt to transaction, do splits from receipt overwrite existing transaction tags?

**Decision**:
- **Receipt splits take precedence** when matching
- Existing transaction-level tags are preserved but marked as `superseded`
- If receipt has splits, those become the source of truth for categorization
- If receipt has no splits (just a total), existing transaction tags remain
- Rationale: Receipt data is more granular and accurate than transaction-level guesses

Merge behavior:
```
Transaction: $87.43 tagged "Shopping"
Receipt: 3 splits (T-shirt $25, Groceries $32, Cleaning $24.99, Tax $5.44)

Result:
- Transaction-level "Shopping" tag → status: superseded
- Split-level tags from receipt → active
- Reports use split-level data when available
```

### 3. Multi-receipt Transactions

Can one transaction have multiple receipts?

**Decision**: **Yes**, with roles:
- `primary` - The main itemized receipt (drives splits)
- `supplementary` - Additional documentation (credit card slip, warranty, etc.)
- Only **one primary receipt** per transaction
- Unlimited supplementary receipts
- If a new receipt is uploaded as primary, the old primary becomes supplementary (with warning)

Use cases:
- Restaurant: itemized bill (primary) + signed credit card slip (supplementary)
- Electronics: store receipt (primary) + warranty card (supplementary)
- Returns: original receipt (primary) + return receipt (supplementary)

```sql
ALTER TABLE receipts ADD COLUMN role TEXT DEFAULT 'primary';
-- role: primary, supplementary
```

## Ollama Models

For this workflow we need:

| Task | Recommended Model | Why |
|------|-------------------|-----|
| Receipt parsing (vision) | `llava:13b` or `llama3.2-vision` | Vision capability, good at structured extraction |
| Merchant classification | `llama3.2:3b` | Fast, good for simple text classification |
| Merchant matching | `llama3.2:3b` | Compare receipt vs transaction merchant names |
| Entity suggestion | `llama3.2:3b` | Context-based inference |

Start with `llama3.2-vision:11b` for vision tasks and `llama3.2:3b` for text tasks. The 3b model is fast enough for interactive use while the 11b vision model handles receipt parsing well.
