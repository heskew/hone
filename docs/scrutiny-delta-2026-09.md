---
title: Scrutiny delta (September 2026)
description: Residual security and privacy risks after the August 2026 pass
date: 2026-09-21
nav_order: 999
---

# Scrutiny delta (September 2026)

Delta only. August backlog #60–#74 is treated as shipped. This note records what still matters for a public, self-hosted, single-user personal-finance box. No code changes ship with this document.

**Health:** Loopback `--no-auth`, MCP resource-server auth, CSRF on `/api`, SQLCipher backups, and RUSTSEC-2026-0285 are in good shape. Two High residuals remain: the Cloudflare email header still authenticates when JWT validation is configured, and a trusted proxy's client IP is taken from the leftmost `X-Forwarded-For` hop.

Reviewed against `main` at `b411082` (PR #117).

## Findings

| ID | Severity | Area | Evidence | Residual risk | Suggested fix shape |
| --- | --- | --- | --- | --- | --- |
| D1 | High | Auth | `crates/hone-server/src/lib.rs:273-303`, `crates/hone-server/src/handlers/auth.rs:60-67` | With `CF_TEAM_NAME` and `CF_AUD_TAG` set, a missing or invalid JWT still accepts `CF-Access-Authenticated-User-Email`. A reachable origin can spoof that header. `/api/me` labels that path `cloudflare_jwt`. | When both JWT settings are set, require a validated JWT. Stash that principal for handlers. |
| D2 | High | Auth | `crates/hone-server/src/lib.rs:558-570`, `docs/deployment.md:214-236` | If the TCP peer is a trusted proxy, the first `X-Forwarded-For` address is the client. Proxies append, so a caller can prepend a `HONE_TRUSTED_NETWORKS` address and skip auth. | Walk `X-Forwarded-For` from the right and skip addresses that are themselves trusted proxies. |
| D3 | Med | Privacy | `crates/hone-server/src/handlers/explore.rs:316-338`, `crates/hone-core/src/ai/orchestrator.rs:57-67`, `docs/FEATURES.md:136` | `ollama_metrics.input_text` stays NULL, and the explore answer plus tool inputs/outputs (transaction rows) are stored in `result_text` and `metadata`. | Store tool name, success, and iteration count. Scrub existing payloads on open. Align the FEATURES sentence. |
| D4 | Med | Privacy | `crates/hone-core/src/ai/host.rs:43-61`, `crates/hone-core/src/ai/ollama.rs:84`, `crates/hone-core/src/ai/anthropic_compat.rs:259` | The remote-AI gate checks the configured host string. AI clients use `reqwest::Client::new()`, which follows redirects, so a local endpoint can 307 a prompt to a public host. | `redirect::Policy::none()` on AI clients. Refuse a resolved address that is not local. |
| D5 | Med | Schema | `crates/hone-core/src/db/mod.rs:208-214`, `docs/FEATURES.md:13` | `run_migrations` is `CREATE TABLE IF NOT EXISTS` plus two privacy `UPDATE`s. A new column on an existing table never reaches a database created earlier. The Pi deploy docs assume that database lives. | On open, fail with the table name when a column this binary reads is missing. Fresh databases still open. |
| D6 | Low | Docker | `Dockerfile:17-21`, `Dockerfile.release:4-12` | `gcr.io/distroless/cc-debian13` with no tag is the root variant (uid 0). The comments say the image runs as nonroot by default. | Use `:nonroot` (uid 65532) and note that `/data` must be writable by that uid. |
| D7 | Low | MCP | `crates/hone-server/src/mcp/oauth.rs:251-264` | RS256 checks build `Validation` from the token's `alg` and, when `kid` is absent, use `keys.first()`. jsonwebtoken 11 has no `none` algorithm and rejects a key-type mismatch, so this is not a demonstrated bypass. | Pin `Algorithm::RS256` and reject tokens with no `kid`. Leave the HS256 `hone mcp-token` path alone. |
| D8 | Low | Crypto | `crates/hone-core/src/backup/local.rs:167-168`, `crates/hone-core/src/db/backup.rs:48-72` | Encrypted backups use the SQLCipher raw key when `HONE_DB_KEY` is set. `BackupInfo.encrypted` is hardcoded `true`, including for a `--no-encrypt` database. | Set the flag from whether the backup was attached with a key. |

### D1 — Cloudflare email header still authenticates when JWT is configured

The middleware doc comment says the email header is trusted only when JWT validation is not configured (`lib.rs:177-180`). `docs/deployment.md:125-129` says the same thing: without JWT config, Hone trusts `CF-Access-Authenticated-User-Email`.

The implementation always accepts a non-empty email header after the JWT block, including when `team_name` is set. A failed JWT (bad signature, wrong `aud`, or a Cloudflare certs fetch error) logs a warning and falls through (`lib.rs:265-303`). `CfPublicKeys` / `cached_keys` is never filled (`lib.rs:67-70`); every JWT attempt fetches certs, and a fetch failure takes the header path.

`GET /api/me` reports `auth_method: "cloudflare_jwt"` whenever both JWT settings are set and the header contains `@`, with no check that a JWT was verified (`handlers/auth.rs:60-67`). Handler `log_audit` calls `get_user_email`, which reads that same header (`lib.rs:627-649`) and does not use the principal the middleware just verified. A valid JWT for one email plus a different email header splits the auth row from the action row.

This matters when the origin port is reachable without Cloudflare stripping `Cf-*` headers. Tunnel-only installs are safe because cloudflared replaces those headers. Configuring JWT does not close the hole the docs tell you JWT closes.

### D2 — Leftmost `X-Forwarded-For` behind a trusted proxy

`get_client_ip` trusts `X-Forwarded-For` only when the TCP peer is in `HONE_TRUSTED_PROXIES` (`lib.rs:535-570`). That part is right. The hop it keeps is the first address. Appending proxies (Traefik, `nginx` `proxy_add_x_forwarded_for`) produce `client-supplied, real-client`. The client-supplied address is what Hone treats as the client.

`docs/deployment.md:203-206` shows `HONE_TRUSTED_NETWORKS=192.168.1.0/24,10.0.0.0/8`, and the k3s section sets `HONE_TRUSTED_PROXIES=10.42.0.0/16`. Together, a request that reaches the proxy can send `X-Forwarded-For: 10.1.1.5` and match the trusted network. Trusted-network auth bypasses every other check (`lib.rs:209-241`).

`X-Real-IP` is the fallback when `X-Forwarded-For` is missing (`lib.rs:573-581`). Prefer the reconstructed client from the forwarded list.

### D3 — Explore metrics persist transaction text

`record_ollama_metric` inserts `input_text` as SQL `NULL` (`db/ollama_metrics.rs:21-26`). Open also clears leftover `input_text` (`db/mod.rs:736-740`). Audit rows for explore store the session id only (`handlers/explore.rs:384-390`). Session transcripts stay in memory (`handlers/explore.rs:71-74`).

The explore metric still writes the model answer to `result_text` and serializes `tool_calls` into `metadata` (`handlers/explore.rs:316-338`). Each `ToolCallRecord` includes `input` and `output` (`ai/orchestrator.rs:57-67`), and tool output is the transaction search result. `docs/ollama.md:309-312` says the metrics detail view shows those inputs and outputs, and that `input_text` does not hold the prompt. `docs/FEATURES.md:136` says prompt/txn text is not persisted. Those two sentences disagree. Classification `result_text` (`tags.rs:385-402`) and receipt-match reasons (`handlers/receipts.rs:617`) are smaller copies of the same pattern.

The rows stay in the local database (SQLCipher when `HONE_DB_KEY` is set). They are a second copy of ledger text in a table the privacy text treats as metrics.

### D4 — Remote-AI gate is the URL string, then redirects

`is_local_ai_host` allows loopback, RFC1918, IPv6 unique-local, `localhost`, `*.local`, single-label names, and `*.docker.internal` (`ai/host.rs:6-15`, `125-148`). Public hosts are refused unless `HONE_ALLOW_REMOTE_AI` is `1`, `true`, or `yes` (`ai/host.rs:24-40`). Integer and hex IPv4 forms are refused. That gate is real and tested.

The HTTP clients are `reqwest::Client::new()` (`ollama.rs:84`, `anthropic_compat.rs:259`, `openai_compatible.rs:89`). The default policy follows redirects. A host that passed the string check can answer `307` to a public URL and the prompt body follows. A single-label or `*.local` name can also resolve to a public address; the check never looks up the IP.

### D5 — Inline schema, long-lived database

`docs/FEATURES.md:13` and `CLAUDE.md` say the schema is inline and there are no migrations while pre-shipping. `run_migrations` matches that: one `execute_batch` of `CREATE TABLE IF NOT EXISTS`, then the `input_text` scrub and the Amex `original_data` scrub (`db/mod.rs:208-214`, `736-767`). There is no `schema_version` and no `ALTER TABLE` in `db/mod.rs`.

New tables appear on next open. A column added to an existing `CREATE TABLE` does not appear on a database created before that column. The failure shows up later, on the first `SELECT` that names it. `docs/deployment.md` and `docs/mcp.md` describe a Pi install with a database under `/home/pi/hone`. That database is the one this gap hits.

### D6 — Distroless comment says nonroot; the tag is root

`Dockerfile:21` and `Dockerfile.release:12` use `gcr.io/distroless/cc-debian13` with no tag. Distroless maps the default tag to the root variant (uid 0). The nonroot variant is `gcr.io/distroless/cc-debian13:nonroot` (uid 65532). Both Dockerfiles say the base runs as nonroot by default (`Dockerfile:17-20`, `Dockerfile.release:4-7`).

Container root is still a container. The inaccurate comment is the part that will mislead the next image change.

### D7 — MCP RS256 validation trusts `header.alg`

Cloudflare JWT validation pins `Algorithm::RS256` (`lib.rs:438`). The external-AS path does `Validation::new(header.alg)` (`mcp/oauth.rs:264`) and, with no `kid`, verifies against `keys.first()` (`mcp/oauth.rs:259-261`). jsonwebtoken 11.0.0 (`Cargo.lock`) has no `none` variant, and a HMAC algorithm with an RSA JWK fails key-type checks. There is no working forge in this tree. The shape is still the one that becomes a bypass if a JWKS ever contains an `oct` key or the crate regresses.

HS256 tokens from `hone mcp-token` are a separate path (`mcp/oauth.rs:165-171`, `lib.rs:499-502`) and stay pinned to HS256.

### D8 — Backup listing always says encrypted

`create_backup` attaches the temp file with `KEY 'x"<hex>"'` when `HONE_DB_KEY` is set, and with `KEY ''` otherwise (`db/backup.rs:48-72`). The key is Argon2id output, hex-encoded, so it is not SQL metacharacters. `LocalDestination::list` then sets `encrypted: true` for every `hone-*` file (`backup/local.rs:167-168`). A development backup of an unencrypted database is reported as encrypted.

## Not findings

Shipped since the August pass, or accepted on purpose. Do not re-open these unless a regression shows up.

- **`--no-auth` off loopback.** `ensure_no_auth_allowed` refuses the flag unless `--host` is `localhost` or an IP whose `is_loopback()` is true. `0.0.0.0` and `::` are refused (`commands/serve.rs:282-306`). Docker compose does not pass the flag (`deploy/docker-compose.yml:17`).
- **MCP auth on `/mcp`.** `create_mcp_router` wraps `/mcp` in the same `auth_middleware` (`mcp/mod.rs:192-256`). Unauthenticated requests get 401 plus `WWW-Authenticate`. RFC 9728 metadata URLs are public on purpose. `HONE_MCP_KEYS` and MCP-audience JWTs are rejected on `/api`; `HONE_API_KEYS` still work on both (`lib.rs:488-513`). That split is documented (`docs/mcp.md:244-247`, `docs/deployment.md:71`).
- **MCP tools are read-only.** The server module states it, and `hone-core/src/tools.rs` has no `INSERT`, `UPDATE`, or `DELETE`. `docs/mcp.md:30` matches the code.
- **MCP tool-call audit gap is explicit.** Auth allow/deny on `/mcp` is written (method, path, via). Tool name and arguments are not (`lib.rs:10-13`, `docs/FEATURES.md:59-61`). Left out on purpose so audit rows stay free of transaction text. A later chip can log the tool name alone.
- **CSRF docs match the code.** `CsrfLayer` is on the `/api` router only (`lib.rs:1067-1069`). Safe methods, trusted `Origin`, `Sec-Fetch-Site: same-origin|none`, and clients that send neither `Origin` nor `Sec-Fetch-Site` pass (`lib.rs:1140-1160`). `docs/deployment.md:80-101` and `docs/mcp.md:255` describe this, including why MCP is unwrapped.
- **API key compare.** `validate_api_key` uses `subtle::ConstantTimeEq` for equal lengths (`lib.rs:516-531`). Empty entries are dropped when the env var is parsed (`commands/serve.rs:274-279`). Length differs, so the compare returns early; that is the usual leftover and not worth a chip.
- **Remote AI opt-in default.** Public `OLLAMA_HOST` / `ANTHROPIC_COMPATIBLE_HOST` values are refused unless `HONE_ALLOW_REMOTE_AI` is set (`ai/host.rs:64-81`). README "local AI" matches that default. D4 is the hole beside it.
- **Prompt text in `input_text` and in `audit_log`.** New metrics force `input_text` NULL and open scrubs leftovers. Explore audit details are the session id. Amex account number, address, and card-member keys are removed from `original_data` on import and again on open (`db/mod.rs:743-767`). The `card_member` column is kept on purpose for filtering (`import.rs:13-24`).
- **SQLCipher key handling.** `HONE_DB_KEY` is required for `Database::new` (`db/mod.rs:90-99`). The passphrase is Argon2id (default `m=19456,t=2,p=1`) with the fixed salt `hone-salt-v1-fix`, then passed as a SQLCipher raw key (`db/mod.rs:45-62`, `114-116`). The fixed salt is what makes the same passphrase open a moved or restored file. The passphrase is not logged. Backups of an encrypted database use that same raw key (`db/backup.rs:55-66`).
- **RUSTSEC-2023-0071 ignore.** `.cargo/audit.toml:10-20` ignores the Marvin RSA advisory because production verifies RS256 with public JWKs (Cloudflare certs, optional `HONE_MCP_JWKS_URL`) and signs MCP tokens with HS256 only (`mcp/oauth.rs:165-171`). The RSA private PEM is in tests (`hone-server/src/tests.rs`). The ignore stays valid. Revisit if Hone signs with RSA or if `rsa` / `jsonwebtoken` ships a fix.
- **RUSTSEC-2026-0285.** `Cargo.lock` has rustls `0.23.45`. PR #117 merged at `b411082`. The advisory is not in `.cargo/audit.toml`.
- **GHCR publish and Trivy.** `build-image` waits on `security-scan-rust` and `security-scan-node`, has `packages: write`, and runs Trivy at `CRITICAL,HIGH` with exit code 1 before push (`.github/workflows/docker.yml:181-258`). `.trivyignore` has no CVE entries. Actions are pinned by SHA.
- **Compose defaults.** The published service binds `0.0.0.0` inside the container, publishes `3000:3000`, and does not set `--no-auth` or a trusted network (`deploy/docker-compose.yml:6-17`). With auth required and no keys, unauthenticated calls get 401. That is a safe default. Binding the host port to `127.0.0.1` is optional hardening, not a hole by itself.
- **README privacy claims that still hold.** No bank credentials (CSV import). Database on disk. Encryption at rest when `HONE_DB_KEY` is set (the Docker snippet sets it). The sentences that over-reach are D3 (FEATURES "txn text is not persisted") and D6 (image comments).

## Proposed backlog

File these as their own issues. Each one is a single change.

### 1. auth: reject the Cloudflare email header when JWT validation is configured

**Why:** D1. Operators who set `CF_TEAM_NAME` and `CF_AUD_TAG` still have a spoofable header path, and `/api/me` calls that path JWT auth. A certs-endpoint failure takes the same path.

**Done when:**

- Both CF variables set, email header present, no valid JWT: `401` on `/api` and `/mcp`.
- A valid RS256 JWT with the configured `aud` and `iss` still authenticates. Wrong `aud`, `iss`, `exp`, or signature does not fall through to the header.
- `/api/me` returns `cloudflare_jwt` only after that JWT check succeeds.
- Handler `log_audit` user matches the middleware principal (request extension or equivalent), so a second email header cannot rename the action row.
- `docs/deployment.md` and the `auth_middleware` doc comment describe this.
- A test covers header-only denial while JWT config is on.

### 2. auth: use the rightmost untrusted X-Forwarded-For hop

**Why:** D2. The documented k3s pair (`HONE_TRUSTED_PROXIES` plus `HONE_TRUSTED_NETWORKS`) lets a client prepend a trusted-network address and skip auth.

**Done when:**

- Client IP is the rightmost `X-Forwarded-For` address that is not itself in `HONE_TRUSTED_PROXIES`.
- Test: peer in `trusted_proxies`, header `10.1.1.5, 203.0.113.9`, trusted network `10.1.1.0/24` → 401.
- Test: the real client address is the one inside the trusted network → auth via `trusted_network`.
- A client-supplied `X-Real-IP` is not preferred over that reconstructed address.
- `docs/deployment.md` trusted-proxy section describes the hop rule.

### 3. privacy: stop storing explore tool payloads in ollama_metrics

**Why:** D3. `input_text` is clean. `result_text` and `metadata.tool_calls[].output` still hold ledger text, and FEATURES says that text is not persisted.

**Done when:**

- New `explore_query` rows store tool name, success, and iteration count. `result_text` is NULL. Tool `input` and `output` strings are absent.
- Opening a database nulls those existing payloads the way `input_text` is already nulled.
- `docs/FEATURES.md` and `docs/ollama.md` describe the same rule.
- A unit test writes a metric from a tool output that contains a distinctive description and asserts that string is not in the row.

### 4. privacy: do not follow redirects on AI HTTP clients

**Why:** D4. `HONE_ALLOW_REMOTE_AI` never sees the second hop.

**Done when:**

- The Ollama, Anthropic-compatible, and OpenAI-compatible clients use `redirect::Policy::none()` (or a policy that refuses a change of host).
- A test double that returns `307` to `https://api.openai.com` does not send the second request.
- After the hostname string passes `is_local_ai_host`, a resolved address that is not loopback, RFC1918, or IPv6 unique-local is refused.
- The existing public-host string tests still pass.

### 5. db: fail closed when the live schema is missing a column this binary reads

**Why:** D5. Pre-shipping "no migrations" is accurate, and a long-lived Pi database will not grow columns by itself.

**Done when:**

- Startup checks the live tables against the columns this binary selects (or a single `schema_version` constant that bumps when a column is added).
- A database missing one of those columns fails in `run_migrations` with an error that names the table and says to export and reset.
- A database created by this binary still opens.
- `docs/FEATURES.md` states that behavior in one sentence.
- No migration framework, no compatibility shim.

### 6. docker: run the distroless image as nonroot

**Why:** D6. The comment and the tag disagree, and the published image runs as root.

**Done when:**

- `Dockerfile` and `Dockerfile.release` use `gcr.io/distroless/cc-debian13:nonroot`.
- The comments say uid 65532, not "nonroot by default".
- `docs/deployment.md` notes that the data volume must be writable by uid 65532.
- The image still serves `/app/ui/dist` and the binary is executable by that user.

### 7. mcp: pin external JWTs to RS256 and require kid

**Why:** D7. The Cloudflare path already pins RS256. The JWKS path trusts the header algorithm. No exploit today; the chip is small and should land before an `oct` key is ever added to a JWKS.

**Done when:**

- `validate_mcp_rs256_with_keys` uses `Validation::new(Algorithm::RS256)` only.
- A token with no `kid` is rejected.
- `hone mcp-token` HS256 tokens still validate on `/mcp` and still fail on `/api`.
- A test token whose header `alg` is not RS256 is rejected against an RSA JWKS.

D8 (backup `encrypted` flag) is real and small. Fold it into whichever change next touches `LocalDestination::list`. It does not need its own issue.

## Tracker note

Issue **#116** is still open. PR **#117** merged (`b411082`, 2026-09-21) and its body says `Refs #116`, so GitHub did not close the issue (`closingIssuesReferences` is empty). `Cargo.lock` rustls is `0.23.45`. Close #116; the advisory work is done.
