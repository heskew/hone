---
name: performance-audit
description: Audits Hone for performance issues. Use when optimizing database queries, analyzing Rust async code, reviewing React rendering, or investigating slow operations.
allowed-tools: Read, Grep, Glob, Bash
---

# Performance Audit for Hone

## Rust Backend (hone-core, hone-server)

### Database Performance (db.rs)
- **N+1 Queries**: Watch for loops that execute queries
- **Missing Indexes**: Ensure indexes on `transactions(account_id)`, `transactions(date)`, `subscriptions(merchant)`
- **Connection Pooling**: Using r2d2, verify pool size is appropriate
- **Batch Operations**: Prefer batch inserts over individual inserts in loops

```rust
// BAD - N+1 query pattern
for account in accounts {
    let txns = get_transactions_for_account(account.id)?; // Query per account!
}

// GOOD - Single query with JOIN or IN clause
let txns = get_all_transactions_for_accounts(&account_ids)?;
```

### Async Performance (hone-server)
- **Blocking in Async**: Never use `std::thread::sleep` in async handlers
- **Database Access**: SQLite is synchronous - use `spawn_blocking` for heavy queries
- **Ollama Calls**: Ensure HTTP client reuses connections

```rust
// BAD - blocks the async runtime
async fn handler() {
    std::thread::sleep(Duration::from_secs(1)); // Blocks!
}

// GOOD - use tokio's async sleep
async fn handler() {
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

### Memory & Allocations
- Avoid `.clone()` in hot paths when references work
- Use `&str` instead of `String` for function parameters when possible
- Pre-allocate vectors with `Vec::with_capacity()` when size is known

## React Frontend (ui/)

### Rendering Performance
- **Unnecessary Re-renders**: Use React DevTools Profiler
- **Missing Keys**: List items need stable, unique keys
- **Heavy Computations**: Wrap in `useMemo` if expensive

```tsx
// BAD - recalculates on every render
const total = transactions.reduce((sum, t) => sum + t.amount, 0);

// GOOD - only recalculates when transactions change
const total = useMemo(
  () => transactions.reduce((sum, t) => sum + t.amount, 0),
  [transactions]
);
```

### Bundle Size
- Tailwind CSS tree-shakes unused styles (good)
- lucide-react supports tree-shaking (import specific icons)
- Check for accidentally importing entire libraries

### API Efficiency
- Paginate large transaction lists
- Debounce search/filter inputs
- Cache responses where appropriate

## Detection Algorithms (detect.rs)

### Zombie Detection
- Group transactions efficiently (use HashMap)
- Avoid re-scanning entire transaction history

### Price Increase Detection
- Index subscriptions by merchant for quick lookup
- Compare only recent transactions (last N months)

### Duplicate Detection
- Pre-categorize subscriptions once, not per-check

## Quick Checks

```bash
# Check for .clone() usage
rg "\.clone\(\)" crates/

# Check for blocking sleep
rg "std::thread::sleep" crates/

# Check bundle size
cd ui && npm run build && ls -la dist/assets/
```
