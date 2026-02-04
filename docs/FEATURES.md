---
title: Features
description: Comprehensive list of implemented features
date: 2026-01-24
---

# Hone Features

Comprehensive list of implemented features.

## Core Features

- Full database layer (schema defined inline, no migrations during development)
- CSV import with auto-detection (web UI and CLI)
- All seven detection algorithms (zombie, price increase, duplicate, auto-cancellation, resume, spending anomaly, tip discrepancy)
- Subscription lifecycle monitoring (auto-detect cancelled, alert on resume)
- CLI with rich output (modular command structure in `commands/`)
- REST API with authentication and audit logging
- 788 Rust tests

## Frontend

React frontend with Tailwind styling:
- Dashboard with stats and widgets:
  - Recent Activity (last 10 transactions)
  - Spending Snapshot (this month vs last month comparison with projection)
  - Top Categories (top 5 spending categories with colored bars)
  - Upcoming Charges (subscriptions due in next 14 days)
  - Insights Widget (proactive AI-generated insights)
- Transaction list with pagination and tags
- Subscription management
- Alert dismissal and restore with "Show dismissed" toggle
- CSV file upload with detection results
- Tag management (tree view, create/edit/delete/merge, rules)
- Reports UI (Recharts: spending charts, trends, merchants, subscriptions)
- Hash-based URL routing (back/forward, refresh, deep links, filter state persistence)
- Dark mode (follows system preference)
- Mobile responsive nav (hamburger menu)

## Authentication

See `docs/deployment.md` for details:
- Cloudflare Access JWT validation (recommended for production)
- Cloudflare Access header (fallback when behind CF Tunnel)
- API keys for machine-to-machine auth (`HONE_API_KEYS`)
- Trusted networks for local access without auth (`HONE_TRUSTED_NETWORKS`)
- Trusted proxies for extracting real client IP (`HONE_TRUSTED_PROXIES`)

## Tags System

Hierarchical tags with 17 root categories and child tags:
- Transport: Gas, Rideshare, Parking, Transit, Tolls, Auto
- Subscriptions: Streaming, Music, CloudStorage, News, Fitness, Gaming, Software
- Root tags: Income, Groceries, Dining, Transport, Healthcare, Shopping, Entertainment, Subscriptions, Travel, Personal, Education, Pets, Gifts, Financial, Utilities, Housing, Other

Auto-tagging priority: rules → patterns → bank category → Ollama → fallback

Merchant name learning: user corrections cached and applied to future imports.

## Reports System

- Spending summary by category with drill-down
- Spending trends (monthly/weekly granularity)
- Top merchants ranking
- Subscription summary with waste breakdown
- Savings report (tracks money saved from cancelled subscriptions)
- Time period presets and custom date ranges
- Entity-based spending reports (by person, pet, vehicle, property)
- Location-based spending reports
- Vehicle cost and mileage tracking
- Click any category to see transactions (drill-down)

## Backup System

See [backup.md](backup.md) for full details.

- Encrypted, compressed backups using SQLCipher's `sqlcipher_export()`
- Local filesystem storage (default: `~/.local/share/hone/backups/`)
- Pluggable destinations via `BackupDestination` trait
- Retention policy with automatic pruning
- Built-in scheduler in server (`HONE_BACKUP_SCHEDULE`)

## Export/Import

- Transaction CSV export with filtering (date range, tag IDs)
- Full JSON backup export (all database tables)
- Full JSON backup import with dependency ordering
- CLI: `hone export transactions`, `hone export full`, `hone import-full`

## Transaction Splits & Entities

See `docs/SPLITS_DESIGN.md` for schema details.

- Entities: people, pets, vehicles, properties for spending attribution
- Transaction splits: break transactions into line items with categories
- Trips: group transactions by event/trip with budgets
- Locations: track where purchases were made
- Mileage logs: track vehicle odometer readings

## Receipt Workflow

See [design/receipts.md](design/receipts.md) for full workflow.

- Receipt-first workflow: upload receipts before bank imports
- AI parsing of receipts via Ollama vision models
- Receipt status tracking (pending → matched/manual_review/orphaned)
- Auto-matching receipts to transactions on import
- Tip discrepancy auto-detection (flags transactions that exceed receipt total)
- Match candidates API for manual linking

## Ollama Integration

- Merchant classification and normalization during import
- Model selection on import (override default model per-import)
- Receipt-to-transaction matching evaluation
- Duplicate detection reasoning
- Spending anomaly explanations with re-analysis
- Metrics tracking (latency, success rate, accuracy)
- AI Metrics page with "Load more" pagination for recent calls
- AI Orchestrator for agentic analysis (optional, uses tool-calling)

## Explore Mode

Conversational interface for querying financial data:
- Chat UI with message history (local state)
- AI-powered natural language queries using tool-calling
- Model selection per-session
- Suggestion chips for common questions
- Requires AI orchestrator configuration (`ANTHROPIC_COMPATIBLE_HOST`, `ANTHROPIC_COMPATIBLE_MODEL`)

## Insight Engine

Proactive AI-powered financial insights:
- **Spending Explainer**: Compares current month vs 3-month baseline
- **Expense Forecaster**: Predicts upcoming expenses
- **Savings Opportunity**: Surfaces zombie/duplicate savings
- Actions: dismiss, snooze (1-90 days), restore, feedback

## Bulk Operations

- Select multiple transactions and apply/remove tags in bulk
- Selection mode toggle with floating bulk action toolbar
- Archive transactions to hide from reports and detection

## Import History

- Every CSV import creates an `ImportSession` record
- Tracks: imported count, skipped duplicates, tagging breakdown, detection results, Ollama model used
- Phase timing for each import step
- Reprocess imports with new models/rules
- Model selection for reprocess (defaults to model used in original import)
- Before/after comparison of reprocessing results
- Historical model comparison across multiple runs
- **Cancel in-progress imports**: Cancel button in import detail modal for stuck/long-running imports
- **Stuck import recovery**: Server automatically marks interrupted imports as failed on startup

## Account Features

- Account-to-person association via `entity_id`
- Amex extended CSV extracts `Card Member` field
- Transaction filtering by account owner or cardholder
- Account-specific subscriptions

## Subscription Management

- Detail modal with overview, alerts, and transactions tabs
- Exclusion learning ("Not a subscription" marks merchant as excluded)
- Zombie alert actions: acknowledge, cancel, or exclude
- Acknowledgment tracking with date display
- Re-acknowledge button for refreshing acknowledgment (prevents stale zombie detection)
- Stale acknowledgment re-check after 90 days (configurable)

## UI Polish

- Desktop min-width (1024px) prevents compressed layouts
- Compact transaction rows on desktop, stacked cards on mobile
- Row hover highlighting across all list views
- Escape key closes modals
- Transaction filtering by tags, date range, entity, cardholder
- Sort by date or amount
- Filter state persisted in URL

## Feedback System

- Thumbs up/down on AI-generated content
- Feedback History page with stats and filters
- Undo/revert capability

## Testing

- Playwright UX integration tests with seeded test data
- Screenshots captured on each test run
- CI workflow for automated testing

## Deployment

- Docker images (multi-arch: amd64, arm64) via GitHub Actions
- Security scanning with Trivy
- See `docs/deployment.md` for guide
