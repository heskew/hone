# Hone

A self-hosted personal finance tool that helps you sharpen your spending habits.

Import your bank transactions, and Hone will analyze your spending patterns to surface insights and identify opportunities to save money.

## Current Features

### Waste Detection
Hone identifies wasteful spending patterns:

- **Zombie Subscriptions** - Recurring charges you may have forgotten about
- **Price Increases** - Services that quietly raised their prices
- **Duplicate Services** - Multiple subscriptions in the same category (e.g., multiple streaming services)
- **Resume Alerts** - Warns when a cancelled subscription starts charging again
- **Auto-Cancellation** - Marks subscriptions as cancelled when expected charges stop appearing

### Transaction Management
- Import transactions from CSV exports (supports major US banks)
- Automatic deduplication prevents double-imports
- Track spending across multiple accounts

### Subscription Tracking
- Automatic detection of recurring charges
- Track monthly subscription costs
- Acknowledge subscriptions you intend to keep
- Cancel and track savings

### Tags & Categories
- Hierarchical tagging system (15 root categories)
- Auto-tagging on import (rules, patterns, optional [Ollama AI](docs/ollama.md))
- Custom tag rules for automatic categorization

### Reports
- Spending by category with drill-down
- Spending trends (monthly/weekly)
- Top merchants ranking
- Subscription summary with waste breakdown
- Savings tracking from cancelled subscriptions

### MCP Server (Conversational Queries)
- Query your finances via LLM tools (Claude Desktop, custom agents)
- All data stays local — MCP server runs on your network
- Read-only access to transactions, spending, subscriptions, alerts
- See [docs/MCP_SERVER.md](docs/MCP_SERVER.md) for setup

## Privacy First

Hone is designed with privacy as a core principle:

- **No bank credentials** - Import transactions via CSV exports from your bank
- **Fully local** - All data stays on your machine in a SQLite database
- **Encrypted at rest** - SQLCipher encryption with Argon2 key derivation
- **Self-hosted** - Run it on your own hardware

## Security Architecture

Hone is designed for deployment behind [Cloudflare Access](https://www.cloudflare.com/products/zero-trust/access/) or a similar authentication proxy.

### Authentication Model

**Production Deployment:**
- Hone expects to run behind Cloudflare Access (or similar reverse proxy)
- Authentication is handled by Cloudflare, which validates users via email OTP
- Cloudflare passes the authenticated user email in the `cf-access-authenticated-user-email` header
- Hone trusts this header when `require_auth: true` (default)

**Important:** Do NOT expose Hone directly to the internet without Cloudflare Access or equivalent protection. The `cf-access-authenticated-user-email` header can be spoofed by anyone who can reach the server directly.

**Local Development:**
- Use `--no-auth` flag to bypass authentication for local development
- This is only safe when the server is bound to localhost

See [docs/deployment.md](docs/deployment.md) for detailed deployment instructions with Cloudflare Tunnel.

## Quick Start

### Prerequisites

- Rust (1.70+)
- Node.js (18+)

### Setup

```bash
# Install UI dependencies
make install-ui

# Initialize the database
make init

# Import sample data (optional - runs tagging and detection automatically)
make sample-import
```

### Development

Run the backend and frontend in separate terminals:

```bash
# Terminal 1 - Backend (http://localhost:3000)
make dev-backend

# Terminal 2 - Frontend (http://localhost:5173)
make dev-ui
```

### Production Build

```bash
make build
cargo run -- serve --port 3000
```

### Encryption (Required by Default)

Database encryption is required by default. Set the `HONE_DB_KEY` environment variable before running any commands:

```bash
# Set your encryption passphrase
export HONE_DB_KEY="your-secure-passphrase"

# Create an encrypted database
hone init
hone import --file statement.csv

# Check encryption status
hone status
```

The passphrase is used to derive an encryption key via Argon2. The same passphrase must be provided for all subsequent operations. Store it securely (e.g., in a password manager or secrets file).

**Development/Testing**: Use `--no-encrypt` to disable encryption (not recommended for real data):

```bash
hone --no-encrypt init
hone --no-encrypt import --file test.csv
```

For Docker deployments, use Docker secrets or mount a key file:

```bash
# Using environment file
echo "HONE_DB_KEY=your-secure-passphrase" > .env
docker run --env-file .env ...
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Frontend (React + Vite)                   │
│                   ui/ - TypeScript + Tailwind                │
└─────────────────────────────────────────────────────────────┘
                              │ /api/*
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     hone-server (Axum)                       │
│                    crates/hone-server/                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                        hone-core                             │
│        crates/hone-core/ - DB, import, detection             │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
         [SQLite]        [CSV Files]     [Ollama API]
                                          (optional)
```

## CLI Reference

```bash
# Setup & Import
hone init                              # Initialize database
hone import --file FILE                # Import CSV (auto-tags and detects)
hone import --file FILE --no-detect    # Import without running detection
hone import --file FILE --no-tag       # Import without auto-tagging
hone detect --kind all                 # Run all detection algorithms
hone detect --kind zombies             # Run specific detection

# Viewing Data
hone dashboard                         # Show summary in terminal
hone status                            # Show database status and encryption
hone accounts                          # List accounts
hone transactions                      # List transactions
hone subscriptions                     # List detected subscriptions
hone alerts                            # List waste alerts

# Tags & Rules
hone tags                              # List all tags
hone tags add Food.FastFood            # Add a child tag
hone rules                             # List tag rules
hone rules add Groceries "WHOLE FOODS" # Add auto-tag rule
hone tag 123 Groceries                 # Tag transaction #123

# Reports
hone report spending                   # Spending by category (this month)
hone report spending --period last-month --expand  # With child categories
hone report trends                     # Monthly spending trends
hone report merchants --limit 20       # Top 20 merchants
hone report subscriptions              # Subscription summary
hone report savings                    # Savings from cancelled subs

# Subscription Management
hone subscriptions cancel Netflix      # Cancel a subscription
hone subscriptions cancel 5 --date 2024-01-15  # Backdate cancellation

# Backup & Restore
hone backup                            # Create backup (hone-backup-YYYY-MM-DD.db)
hone backup --output ~/backups/hone.db # Backup to specific path
hone restore --input backup.db         # Restore (fails if db exists)
hone restore --input backup.db --force # Restore and overwrite existing

# Server
hone serve --port 3000                 # Start the web server
hone serve --no-auth                   # Start without auth (dev only)
hone serve --mcp-port 3001             # Enable MCP server for LLM access
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/dashboard` | Dashboard statistics |
| GET | `/api/accounts` | List all accounts |
| POST | `/api/accounts` | Create a new account |
| GET | `/api/transactions` | List transactions (paginated) |
| GET | `/api/subscriptions` | List detected subscriptions |
| POST | `/api/subscriptions/:id/acknowledge` | Mark subscription as known |
| POST | `/api/subscriptions/:id/cancel` | Cancel subscription |
| GET | `/api/alerts` | List waste alerts |
| POST | `/api/alerts/:id/dismiss` | Dismiss an alert |
| POST | `/api/detect` | Run waste detection |
| POST | `/api/import` | Import CSV (runs tagging + detection) |
| GET | `/api/tags` | List all tags |
| GET | `/api/tags/tree` | Get tag hierarchy |
| GET | `/api/reports/spending` | Spending by category |
| GET | `/api/reports/trends` | Spending trends |
| GET | `/api/reports/merchants` | Top merchants |
| GET | `/api/reports/subscriptions` | Subscription summary |
| GET | `/api/reports/savings` | Savings report |
| POST | `/mcp` | MCP server endpoint (on `--mcp-port`) |

## Project Structure

```
hone/
├── crates/
│   ├── hone-core/     # Shared library (db, models, import, detection)
│   ├── hone-cli/      # Command-line interface
│   └── hone-server/   # REST API server (Axum)
├── ui/                # React frontend
├── samples/           # Sample CSV data for testing
└── Makefile           # Development commands
```

## Docker Deployment

Docker images are automatically built for ARM64 and AMD64 and published to GitHub Container Registry.

```bash
# On Raspberry Pi (or any Docker host)
mkdir -p ~/hone && cd ~/hone
curl -O https://raw.githubusercontent.com/heskew/hone/main/deploy/docker-compose.yml

# Create .env with your secrets
cat > .env << 'EOF'
HONE_DB_KEY=your-secure-passphrase
CLOUDFLARE_TUNNEL_TOKEN=your-tunnel-token
EOF

# Deploy
docker compose up -d
```

See [docs/deployment.md](docs/deployment.md) for the full Raspberry Pi deployment guide.

## Roadmap

- [x] File upload in web UI
- [x] CSV format auto-detection
- [x] Hierarchical tags with auto-tagging
- [x] Ollama integration for merchant classification
- [x] Reports system (spending, trends, merchants, savings)
- [x] Backup/restore functionality
- [x] Tag management UI (tree view, create/edit/delete, rules)
- [x] Reports UI (spending charts, trends, merchants, subscription summary)
- [x] URL routing (back/forward, refresh, deep links)
- [x] Docker deployment with CI/CD
- [x] MCP server for conversational queries via LLMs

## License

[MIT](LICENSE)
