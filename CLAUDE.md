# CLAUDE.md - Development Context for Hone

This file provides context for AI-assisted development sessions.

## Project Overview

Hone is a self-hosted personal finance tool that detects wasteful spending:
- Zombie subscriptions (forgotten recurring charges)
- Price increases (services that quietly raised prices)
- Duplicate services (multiple streaming/storage/etc)

## Architecture

See `README.md` for architecture diagram. Stack: React + Vite frontend, Axum REST API, SQLite database.

## Crate Structure

- **hone-core**: Shared library
  - `db/` - SQLite with r2d2 pooling, SQLCipher encryption
  - `backup/` - Pluggable backup system (local filesystem, R2 stub)
  - `models.rs` - Domain types
  - `import.rs` - CSV parsers for major US banks
  - `export.rs` - Transaction CSV export, full JSON backup/restore
  - `detect.rs` - Waste detection algorithms
  - `tags.rs` - Tag assignment engine
  - `ai/` - Pluggable local AI backend (Ollama, OpenAI-compatible, Mock)
  - `prompts.rs` - Prompt library with override support
  - `model_router.rs` - Task-based model routing with health tracking
  - `context.rs` - Context assembler for LLM prompts
  - `training.rs`, `training_pipeline.rs` - Fine-tuning infrastructure
  - `insights/` - Proactive financial insights engine

- **hone-cli**: Command-line interface
  - `commands/` - Command implementations split by domain
  - Commands: init, import, detect, serve, dashboard, status, accounts, transactions, subscriptions, alerts, entities, tags, rules, tag, untag, report, receipts, ollama, prompts, training, export, import-full, backup, rebuild, reset

- **hone-server**: Axum REST API
  - `lib.rs` - Server config, router, auth middleware, security headers
  - `handlers/` - Domain-specific HTTP handlers (22 modules)
  - `mcp/` - MCP server for LLM tool access (enable with `--mcp-port 3001`)
  - Cloudflare Access authentication, audit logging

## Key Design Decisions

1. **CSV-only import** - No bank credentials stored, privacy-first
2. **Encryption required** - SQLCipher encryption by default (`HONE_DB_KEY`); `--no-encrypt` for dev
3. **Local AI only** - Ollama and OpenAI-compatible servers. No cloud APIs
4. **Deduplication** - SHA256 hash of (date, description, amount) prevents double-imports
5. **Secure by default** - Cloudflare Access auth required; `--no-auth` for local dev
6. **Audit logging** - All API access logged
7. **Full processing by default** - Import runs tagging + detection automatically
8. **Raw data preservation** - Original CSV data stored as JSON for reprocessing

## Development Stage

**Pre-shipping**: We are NOT yet at the "shipping" stage. This means:
- **No migrations required** - Schema changes can be made directly; just reset the database
- **No backwards compatibility** - APIs, data formats, and behavior can change freely
- **No deprecation cycles** - Old code/approaches can be removed immediately

When we explicitly agree to enter the "shipping" stage, we'll add migrations and maintain backwards compatibility.

## Database Schema

Key tables (see `docs/SPLITS_DESIGN.md` for entity/split schema):
- `accounts` - Bank accounts with optional `entity_id`
- `transactions` - Individual charges (with `archived`, `original_data`, `import_format`)
- `subscriptions` - Detected recurring patterns with `account_id`
- `alerts` - Waste detection findings
- `tags` - Hierarchical category tags (17 root tags seeded)
- `transaction_tags` - Transaction-to-tag mapping with source tracking
- `tag_rules` - User-defined patterns for auto-tagging
- `audit_log` - API access history
- `entities` - People, pets, vehicles, properties
- `locations`, `trips`, `mileage_logs` - Location and travel tracking
- `receipts` - Receipt storage with status workflow
- `merchant_*_cache` - Learned merchant classifications, names, tags
- `import_sessions`, `import_skipped_transactions` - Import tracking
- `user_feedback` - Feedback on AI-generated content
- `training_experiments` - Fine-tuning experiment tracking
- `reprocess_runs`, `reprocess_snapshots` - Reprocess comparison
- `insight_findings` - Proactive financial insights
- `ollama_metrics` - AI call tracking (latency, success, tool calls metadata for explore queries)

## Current State

See `docs/FEATURES.md` for comprehensive feature list.

**Test coverage**: 788 Rust tests, Playwright UX tests

**Not yet implemented**: Cloud backup to Cloudflare R2 (stub ready)

## Detection Algorithms

See `docs/DETECTION.md` for algorithm details. Seven algorithms:
- Zombie Detection - forgotten recurring charges
- Price Increase Detection - subscription price changes
- Duplicate Detection - multiple services in same category
- Auto-Cancellation Detection - stopped subscriptions
- Resume Detection - reactivated subscriptions
- Spending Anomaly Detection - unusual spending patterns
- Tip Discrepancy Detection - bank amount higher than receipt total

## AI Integration

See `docs/ollama.md` for setup and configuration.

**Three modes:**
1. **Classification Mode** (`OLLAMA_HOST`) - merchant classification, normalization, subscription detection
2. **Agentic Mode** (`ANTHROPIC_COMPATIBLE_HOST`) - tool-calling for deeper analysis
3. **Explore Mode** - conversational interface using agentic mode to answer financial questions
   - Multi-turn conversations with session persistence
   - Model selector to switch between available Ollama models at runtime
   - Queries tracked in AI Metrics as `explore_query` operations with tool call history

**Prompts**: Stored in `prompts/` with override support (`~/.local/share/hone/prompts/overrides/`)

**Model routing**: Config in `config/models.toml` with task-based routing and health tracking

## Development Commands

```bash
# Build
make build

# Run backend (port 3000)
export HONE_DB_KEY="your-passphrase-here"
cargo run -- serve --port 3000

# Run frontend dev server (port 5173)
cd ui && npm run dev

# Enable Ollama
export OLLAMA_HOST="http://localhost:11434"
export OLLAMA_MODEL="gemma3"

# Quick test flow
cargo run -- --no-encrypt init
cargo run -- --no-encrypt import --file samples/sample.csv
cargo run -- --no-encrypt dashboard

# Reset database
cargo run -- --no-encrypt reset --soft -y

# Run tests
cd ui && npm run test:ux
```

## UI Structure

```
ui/src/
├── components/
│   ├── common/         # Shared UI components
│   ├── Dashboard/      # Dashboard view
│   ├── Transactions/   # Transaction list, detail tabs
│   ├── Subscriptions/  # Subscription management
│   ├── Alerts/         # Alert cards with detail modal
│   ├── Insights/       # Proactive insights
│   ├── Import/         # CSV import, history
│   ├── Tags/           # Tag tree management
│   ├── Reports/        # Charts and reports (Recharts)
│   ├── Receipts/       # Receipt upload, linking
│   ├── Ollama/         # AI Metrics page (all AI call tracking)
│   ├── Feedback/       # Feedback history page
│   └── Explore/        # Conversational query interface with model selector
├── hooks/              # Custom React hooks
├── App.tsx             # Layout and routing
├── api.ts              # API client
└── types.ts            # TypeScript interfaces
```

## Code Style

- Rust: Follow standard idioms, use Result types, propagate errors with `?`
- TypeScript: Strict mode, prefer named exports
- CSS: Tailwind utilities, custom components in index.css
- Comments: Explain "why" not "what"

## UI Design Principles

- Clean, confident typography - numbers feel important
- Semantic color: green=income, amber=attention, red=waste (not all expenses)
- Normal expenses use neutral colors
- Subtle motion - alive but not distracting
- Satisfying interactions
- No clutter - every element earns its place

## Dark Mode Patterns

Tailwind dark mode uses `dark:` prefix (follows system preference).

- Primary text: `text-hone-900 dark:text-hone-100`
- Secondary text: `text-hone-600 dark:text-hone-400`
- Cards: `bg-white dark:bg-hone-900` (via `.card` class)
- Table headers: `bg-hone-50 dark:bg-hone-800`
- Hover: `hover:bg-hone-50 dark:hover:bg-hone-800`

## Test Data Guidelines

- Major public company names are fine (Netflix, Amazon, Costco)
- Avoid small regional/local business names
- Use fictional names for non-public entities

## Deployment

See `docs/deployment.md` for Docker deployment guide.

Docker images: `ghcr.io/heskew/hone-money` (multi-arch: amd64, arm64)
