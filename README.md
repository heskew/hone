<p align="center">
  <img src="site/favicon.svg" width="64" height="64" alt="Hone">
</p>

<h1 align="center">$HONE</h1>

<p align="center">
  Personal finance software you actually own.<br>
  <a href="https://hone.money">hone.money</a>
</p>

---

I built this for myself. It finds the subscriptions I forgot about, the services that quietly raised prices, the duplicate apps I'm paying for.

It runs on my machine. My bank doesn't know about it. Neither does anyone else.

**No cloud. No subscriptions. No surveillance.**

Use it if you want. Fork it. Or just know that software like this is possible.

## What It Does

- **Zombie Subscriptions** - Recurring charges you forgot about
- **Price Increases** - Services that quietly raised their prices
- **Duplicate Services** - Multiple subscriptions in the same category
- **Spending Reports** - See where your money actually goes

## How It Works

1. Export transactions from your bank as CSV
2. Import into Hone
3. Detection algorithms find the waste
4. Optional: Add local AI (Ollama) for smarter merchant classification

## Privacy First

- **No bank credentials** - CSV import only
- **Fully local** - SQLite database on your machine
- **Encrypted at rest** - SQLCipher with Argon2 key derivation
- **Self-hosted** - Run it on your own hardware

## Quick Start

```bash
# Prerequisites: Rust 1.70+, Node.js 18+

# Build
make install-ui
make build

# Set encryption key
export HONE_DB_KEY="your-secure-passphrase"

# Initialize and run
hone init
hone serve --port 3000
```

For development:
```bash
make dev-backend  # Terminal 1 - API on :3000
make dev-ui       # Terminal 2 - UI on :5173
```

## Docker

```bash
docker pull ghcr.io/heskew/hone-money:latest

docker run -d \
  -p 3000:3000 \
  -v hone-data:/data \
  -e HONE_DB_KEY="your-secure-passphrase" \
  ghcr.io/heskew/hone-money:latest
```

## Architecture

```
React + Vite (ui/)
       │
       ▼
Axum REST API (hone-server)
       │
       ▼
Core Library (hone-core)
       │
   ┌───┴───┐
SQLite   Ollama (optional)
```

## License

[MIT](LICENSE)
