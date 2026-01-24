---
title: Status
description: What's working in Hone today
date: 2026-01-24
---

What's working in Hone today.

## Core Features

- CSV import with auto-detection (Chase, BofA, Amex, Capital One, BECU)
- Transaction deduplication (SHA256 hash)
- Subscription detection (3+ transactions, 5% variance, 70% interval consistency)
- Six detection algorithms: zombie, price increase, duplicate, auto-cancellation, resume, spending anomaly
- Hierarchical tags with auto-tagging pipeline
- Reports: spending, trends, merchants, subscriptions, savings
- Transaction archiving and splits

## Infrastructure

- SQLite + SQLCipher encryption (required by default)
- Axum REST API with Cloudflare Access auth
- Full audit logging
- Docker multi-arch images (amd64, arm64)
- Local encrypted backups with CLI management

## UI

- Dashboard with spending snapshot and upcoming charges
- Transactions with search, filter, sort, and tagging
- Subscriptions with detail modal and lifecycle tracking
- Alerts for waste detection findings
- Tags management with tree view and rules
- Reports with drill-down to transactions
- Receipt upload and matching workflow
- Explore mode for conversational queries
- Dark mode, mobile responsive

## AI Integration (Ollama)

- Merchant classification and normalization
- Subscription vs retail pre-filtering
- Metrics and health monitoring
- Transaction reprocessing with different models
- Prompt library with user overrides
- Model router with task-based routing
- Explore mode with tool-calling

## Learning System

- Merchant name cache (AI results cached, user edits take priority)
- Tag learning (manual tags create merchant mappings)
- User feedback system with ratings and corrections
- Feedback injected into prompts for improvement

## Testing

- 788 Rust tests
- Playwright UX tests

## Known Limitations

- Price increase detection requires 90+ days of history
- Subscription detection works best with consistent merchant names
