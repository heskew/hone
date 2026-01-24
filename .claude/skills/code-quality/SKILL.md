---
name: code-quality
description: Enforces code quality standards for Hone. Use when writing new code, reviewing changes, or refactoring existing code.
allowed-tools: Read, Grep, Glob, Bash
---

# Code Quality Standards for Hone

## Rust Guidelines

### Error Handling
- Use `thiserror` for library errors (hone-core)
- Use `anyhow` for application errors (hone-cli, hone-server)
- Propagate errors with `?`, don't `.unwrap()` in production code

```rust
// GOOD - propagate with context
let conn = pool.get().context("Failed to get database connection")?;

// BAD - panics on error
let conn = pool.get().unwrap();
```

### Naming Conventions
- Types: `PascalCase` (e.g., `Transaction`, `AlertKind`)
- Functions/variables: `snake_case` (e.g., `get_transactions`, `account_id`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `DEFAULT_PORT`)

### Code Organization
- Keep functions focused and small (< 50 lines ideal)
- Group related functionality in modules
- Public API at top of file, private helpers below

### Documentation
- Document public APIs with `///` doc comments
- Explain "why" not "what" in inline comments
- Keep CLAUDE.md updated with architectural decisions

## TypeScript/React Guidelines

### Type Safety
- Strict mode enabled in tsconfig.json
- Avoid `any` - use proper types or `unknown`
- Define interfaces for API responses (see `ui/src/types.ts`)

```typescript
// GOOD - typed response
interface Transaction {
  id: number;
  amount: number;
  date: string;
}

// BAD - loses type safety
const data: any = await response.json();
```

### Component Structure
- One component per file
- Props interface defined above component
- Hooks at top of component body

### CSS/Styling
- Use Tailwind utilities
- Custom styles in `index.css` only when Tailwind can't do it
- Semantic color usage: green=savings, amber=attention, red=waste

## Project Structure

```
crates/
  hone-core/     # Shared library (no CLI/server deps)
  hone-cli/      # CLI binary (depends on core)
  hone-server/   # Web server (depends on core)
ui/              # React frontend (independent)
```

- Core logic belongs in `hone-core`
- CLI and server should be thin wrappers
- Frontend communicates only via REST API

## Quality Checks

```bash
# Rust formatting
cargo fmt --check

# Rust linting
cargo clippy -- -D warnings

# TypeScript checking
cd ui && npm run lint

# Build check
cargo build && cd ui && npm run build
```

## Before Committing

1. `cargo fmt` - format Rust code
2. `cargo clippy` - check for common mistakes
3. `cargo test` - run tests (when added)
4. `cd ui && npm run lint` - check TypeScript
5. `cd ui && npm run build` - verify frontend builds
