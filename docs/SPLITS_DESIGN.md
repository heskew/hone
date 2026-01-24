---
title: Splits & Entities
description: Transaction splits and entity tracking schema
date: 2026-01-24
---

# Transaction Splits & Entities Design

## Overview

Extend Hone to support splitting transactions into line items, tracking who/what spending is for (entities), and capturing location data. This enables answering questions like:

- "How much do we spend on each kid?"
- "What's our total pet care cost?"
- "How much did the Paris trip cost?"
- "What are the true costs of owning the lake house?"

## Core Concepts

### Entities

People, pets, vehicles, or properties that spending can be attributed to.

| Type | Examples | Typical Spending |
|------|----------|------------------|
| `person` | Marcus, Sarah, "The Kids" | Clothing, activities, personal items |
| `pet` | Fluffy, Rex | Vet, food, toys, grooming |
| `vehicle` | Honda Civic, F-150 | Gas, maintenance, insurance, registration |
| `property` | Main house, Lake cabin | Repairs, utilities, furnishings, taxes |

### Splits

Breaking one bank transaction into multiple line items, each with its own:
- Amount
- Description
- Category (tag)
- Entity (who/what it's for)
- Purchaser (who bought it)

### Locations

Where transactions occurred and where vendors are based.

## Database Schema

### New Tables

```sql
-- Household entities (people, pets, vehicles, properties)
CREATE TABLE entities (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT NOT NULL,  -- person, pet, vehicle, property
    icon TEXT,           -- emoji or icon name
    color TEXT,          -- hex color for UI
    archived BOOLEAN DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Locations (reusable across transactions)
CREATE TABLE locations (
    id INTEGER PRIMARY KEY,
    name TEXT,                    -- "Home", "Target on 5th", "Paris"
    address TEXT,
    city TEXT,
    state TEXT,
    country TEXT DEFAULT 'US',
    latitude REAL,
    longitude REAL,
    location_type TEXT,           -- home, work, store, online, travel
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Transaction line items (splits)
CREATE TABLE transaction_splits (
    id INTEGER PRIMARY KEY,
    transaction_id INTEGER NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    amount REAL NOT NULL,
    description TEXT,             -- "T-shirt", "Paper towels"
    split_type TEXT NOT NULL DEFAULT 'item',  -- item, tax, tip, fee, discount, rewards
    entity_id INTEGER REFERENCES entities(id),      -- who/what it's for (NULL = household)
    purchaser_id INTEGER REFERENCES entities(id),   -- who bought it (NULL = account owner)
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_splits_transaction ON transaction_splits(transaction_id);
CREATE INDEX idx_splits_entity ON transaction_splits(entity_id);

-- Splits have their own tags (independent of transaction-level tags)
CREATE TABLE split_tags (
    split_id INTEGER NOT NULL REFERENCES transaction_splits(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    source TEXT NOT NULL DEFAULT 'manual',  -- manual, pattern, ollama, rule
    confidence REAL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (split_id, tag_id)
);

-- Receipt storage for AI parsing
CREATE TABLE receipts (
    id INTEGER PRIMARY KEY,
    transaction_id INTEGER REFERENCES transactions(id) ON DELETE CASCADE,
    image_data BLOB,              -- for small receipts stored in DB
    image_path TEXT,              -- for file-based storage
    parsed_json TEXT,             -- cached LLM output
    parsed_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_receipts_transaction ON receipts(transaction_id);
```

### Transaction Table Updates

```sql
ALTER TABLE transactions ADD COLUMN purchase_location_id INTEGER REFERENCES locations(id);
ALTER TABLE transactions ADD COLUMN vendor_location_id INTEGER REFERENCES locations(id);
```

## Split Types

| Type | Amount | Description |
|------|--------|-------------|
| `item` | Usually positive | Products or services purchased |
| `tax` | Positive | Sales tax, VAT, etc. |
| `tip` | Positive | Gratuity for service |
| `fee` | Positive | Delivery, service, convenience fees |
| `discount` | Negative | Coupons, promo codes, loyalty discounts |
| `rewards` | Negative | Points redeemed, cashback applied |

## Examples

### Restaurant Delivery

```
DOORDASH $47.82
├── $28.00 - Pad Thai        [item]     Dining      for: Self
├── $14.00 - Green Curry     [item]     Dining      for: Partner
├── $3.50  - Tax             [tax]      Dining
├── $5.99  - Delivery fee    [fee]      Dining
├── $4.00  - Tip             [tip]      Dining
└── -$7.67 - Promo code      [discount] Dining
    ─────────
    $47.82 ✓
```

### Target Run

```
TARGET $87.43
├── $25.00 - T-shirt           [item]  Clothing    for: Son
├── $32.00 - Groceries         [item]  Groceries   for: Household
├── $24.99 - Cleaning supplies [item]  Household   for: Household
├── $2.19  - Tax (clothing)    [tax]   Clothing
└── $3.25  - Tax (household)   [tax]   Household
    ─────────
    $87.43 ✓
```

### Pet Store

```
PETCO $156.78
├── $89.99 - Dog food (large)  [item]  Pet Care    for: Rex
├── $45.00 - Cat litter        [item]  Pet Care    for: Whiskers
├── $12.99 - Dog treats        [item]  Pet Care    for: Rex
└── $8.80  - Tax               [tax]   Pet Care
    ─────────
    $156.78 ✓
```

### Vacation Hotel

```
MARRIOTT PARIS $1,247.50
Location: Paris, France

├── $1,050.00 - Room (3 nights) [item]  Travel/Lodging  for: Household
├── $142.50   - Taxes & fees    [tax]   Travel
└── $55.00    - Room service    [item]  Dining          for: Household
    ─────────
    $1,247.50 ✓
```

## Business Rules

### Split Validation

1. **Splits must sum to transaction amount** - enforced at application level with tolerance for rounding (±$0.01)
2. **Splits are optional** - transactions without splits work as they do today
3. **Entity is nullable** - NULL means "household/shared"
4. **Purchaser is nullable** - NULL means account owner (inferred from account)

### Tag Hierarchy

When a transaction has splits:
- **Split-level tags** are used for detailed reporting
- **Transaction-level tags** remain for backward compatibility and unsplit transactions
- Reports can aggregate at either level

### Location Handling

- **Purchase location**: Where you physically were (or delivery address for online)
- **Vendor location**: Where the business is based
- Both are optional and at transaction level (not split level)
- Common locations (Home, Work) can be saved and reused

## Reports Enabled

### By Entity

- "Total spending for [Son] this year"
- "Pet care costs by pet"
- "Vehicle costs (gas + maintenance + insurance) per vehicle"
- "Property expenses for lake house"

### By Split Type

- "Total sales tax paid this year"
- "Delivery fees by month"
- "Tips as percentage of dining spending"
- "Savings from discounts/coupons"

### By Location

- "Spending during Paris trip" (purchase location = Paris, date range)
- "Local vs. online shopping"
- "Spending by city"

### Combined

- "Son's clothing spending by month"
- "Pet healthcare costs by pet"
- "Vehicle fuel costs per mile" (with mileage tracking on entity)

## AI Integration

### Receipt Parsing

1. User uploads receipt photo (or forwards email receipt)
2. LLM extracts line items with:
   - Description
   - Amount
   - Suggested category
   - Suggested entity (based on context: "kids clothes" → child entity)
3. User reviews and adjusts
4. Splits created

### Smart Defaults

- Learn that certain merchants often need splitting (Target, Costco, Amazon)
- Learn that certain merchants never need splitting (Netflix, Spotify)
- Suggest entities based on merchant + category patterns

### Entity Inference

- "PETCO" → likely for a pet entity
- "GAMESTOP" → likely for a child entity
- "JIFFY LUBE" → likely for a vehicle entity

## UI Considerations

### Transaction List

- Show split indicator icon for transactions with splits
- Expand to show split details inline or in modal
- Quick actions: "Split this", "Add receipt"

### Split Entry

- Quick mode: Just amounts + categories (tax lumped in)
- Detailed mode: Full line items with all fields
- Receipt scan: AI-populated, user confirms

### Entity Management

- Simple CRUD for entities
- Archive instead of delete (preserve historical data)
- Color/icon picker for visual distinction

### Location Management

- Save frequent locations (Home, Work, etc.)
- Auto-suggest from transaction description when available
- Optional - not required for basic usage

## Implementation Phases

### Phase 1: Foundation ✅

- [x] Database schema for new tables
- [x] Entity CRUD (API + CLI)
- [x] Location CRUD (API + CLI)
- [x] Basic split creation (API + CLI)

### Phase 2: Core Functionality ✅

- [ ] Split entry UI (transaction detail view) - *backend complete, UI pending*
- [x] Split tags support
- [x] Update reports to use split data when available
- [x] Entity-based filtering in reports

### Phase 3: AI Enhancement ✅

- [x] Receipt storage (database + API)
- [x] LLM receipt parsing (via Ollama) - *backend complete*
- [x] Smart entity suggestions (via Ollama)
- [x] Split recommendations for known multi-category merchants

### Phase 4: Advanced Features ✅

- [x] Location-based reporting
- [x] Trip/event grouping (with budgets)
- [x] Vehicle mileage tracking
- [x] Property expense summaries
- [x] 266 tests with 84.5% code coverage
