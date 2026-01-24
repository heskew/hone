---
title: Deployment
description: Deploy Hone with Docker
date: 2026-01-24
---

Deploy Hone with Docker.

## Quick Start

### 1. Get the files

```bash
mkdir -p ~/hone && cd ~/hone
curl -O https://raw.githubusercontent.com/heskew/hone/main/deploy/docker-compose.yml
curl -O https://raw.githubusercontent.com/heskew/hone/main/deploy/.env.example
mv .env.example .env
```

### 2. Configure

Edit `.env` and set your database encryption key:

```
HONE_DB_KEY=your-secure-passphrase
```

### 3. Deploy

```bash
docker compose pull
docker compose up -d
```

Access Hone at http://localhost:3000

## Configuration

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `HONE_DB_KEY` | Yes | Database encryption passphrase |
| `OLLAMA_HOST` | No | Ollama server URL for AI features |
| `OLLAMA_MODEL` | No | Ollama model (default: gemma3) |
| `CF_TEAM_NAME` | Recommended | Cloudflare team name for JWT validation |
| `CF_AUD_TAG` | Recommended | Cloudflare Access application audience tag |
| `HONE_API_KEYS` | No | Comma-separated API keys for internal service auth |
| `HONE_TRUSTED_NETWORKS` | No | Comma-separated IPs/CIDRs that bypass auth |
| `HONE_TRUSTED_PROXIES` | No | Comma-separated proxy IPs/CIDRs to trust X-Forwarded-For from |

### Authentication

Hone supports four authentication methods:

1. **Cloudflare Access JWT** (recommended) - Cryptographically validates `Cf-Access-Jwt-Assertion` header
2. **Cloudflare Access header** (fallback) - Trusts `CF-Access-Authenticated-User-Email` header
3. **API Keys** - For internal services, use `Authorization: Bearer <key>` header
4. **Trusted Networks** - Requests from configured IP addresses/subnets bypass auth

For local/development use, add `--no-auth` to the command:

```yaml
command: ["serve", "--host", "0.0.0.0", "--db", "/data/hone.db", "--no-auth"]
```

### Security Requirements

**Important**: Enable JWT validation for production deployments behind Cloudflare Access.

#### JWT Validation (Recommended)

When `CF_TEAM_NAME` and `CF_AUD_TAG` are configured, Hone validates the JWT in the
`Cf-Access-Jwt-Assertion` header against Cloudflare's public keys. This provides
cryptographic proof that requests came through Cloudflare Access.

1. Get your **team name** from the Cloudflare Zero Trust dashboard URL:
   `https://one.dash.cloudflare.com/<account-id>/<team-name>/...`

2. Get your **audience tag** from Access > Applications > your app > Overview:
   Look for "Application Audience (AUD) Tag"

3. Add to `.env`:
   ```
   CF_TEAM_NAME=your-team-name
   CF_AUD_TAG=your-aud-tag-here
   ```

#### Header-Only Authentication (Fallback)

Without JWT config, Hone trusts the `CF-Access-Authenticated-User-Email` header.
This is safe **only** when behind Cloudflare Tunnel, which strips and rewrites
CF headers. If you bypass Cloudflare, anyone can spoof these headers.

**Safe configurations:**
- Behind Cloudflare Tunnel + Access with JWT validation (production, recommended)
- Behind Cloudflare Tunnel + Access without JWT validation (production, less secure)
- Local network only + trusted networks (local browser access)
- Local network only + API keys (internal services)
- `--no-auth` on localhost only (development)

**Unsafe configurations:**
- Port forwarded to internet without Cloudflare
- Exposed to untrusted networks with auth enabled but no Cloudflare

### API Key Setup

For machine-to-machine auth (e.g., Mac training script accessing Pi server):

1. Generate an API key (64 hex chars = 256 bits):
   ```bash
   openssl rand -hex 32
   ```

2. Add to `.env` on the Pi:
   ```
   HONE_API_KEYS=your-generated-key
   ```

3. Use the key in requests:
   ```bash
   curl -H "Authorization: Bearer your-generated-key" http://pi:3000/api/training/tasks
   ```

**API key security notes:**
- Keys are compared using constant-time comparison (timing attack resistant)
- Store keys securely; treat them like passwords
- Rotate keys if compromised
- Multiple keys supported (comma-separated) for key rotation

### Trusted Networks Setup

For local network access without authentication (e.g., accessing Hone from your home network):

1. **Identify your local network CIDR** (e.g., `192.168.1.0/24` for most home networks)

2. **Add to `.env`:**
   ```
   HONE_TRUSTED_NETWORKS=192.168.1.0/24
   ```

3. **Multiple networks/IPs** (comma-separated):
   ```
   HONE_TRUSTED_NETWORKS=192.168.1.0/24,10.0.0.0/8,172.16.0.5
   ```

**Trusted networks security notes:**
- Only use for networks you fully trust (e.g., home LAN behind firewall)
- Individual IPs are automatically treated as /32 (IPv4) or /128 (IPv6)
- Client IP is determined from the TCP connection by default (X-Forwarded-For is NOT trusted unless from a trusted proxy)
- Combine with Cloudflare Access for remote access while allowing local network bypass

### Trusted Proxies Setup

When Hone runs behind a reverse proxy (e.g., Traefik in k3s, nginx), it sees the proxy's IP instead of the real client IP. To get the real client IP from `X-Forwarded-For` headers, configure the proxy as trusted:

1. **Identify your proxy's IP/CIDR** (e.g., `10.42.0.0/16` for k3s pod network)

2. **Add to `.env`:**
   ```
   HONE_TRUSTED_PROXIES=10.42.0.0/16
   ```

3. **For k3s/Kubernetes** with Traefik:
   ```yaml
   env:
     - name: HONE_TRUSTED_PROXIES
       value: "10.42.0.0/16"
   ```

**Trusted proxies security notes:**
- Only trust proxies you control (never trust arbitrary IPs)
- X-Forwarded-For is only parsed when the TCP connection comes from a trusted proxy
- This is required for trusted networks to work behind reverse proxies
- Common proxy CIDRs: `10.42.0.0/16` (k3s), `10.244.0.0/16` (standard k8s), `172.17.0.0/16` (Docker)

## Network Isolation

Hone only makes outbound connections to configured AI hosts (Ollama). There's no telemetry, no cloud APIs. For defense-in-depth, you can restrict egress at the network level.

### Kubernetes - NetworkPolicy

Use a NetworkPolicy to restrict Hone's egress to only:
1. DNS (for hostname resolution)
2. Your Ollama host on port 11434

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: hone-egress
  namespace: default  # Change to your namespace
spec:
  podSelector:
    matchLabels:
      app: hone
  policyTypes:
    - Egress
  egress:
    # Allow DNS resolution
    - to:
        - namespaceSelector: {}
          podSelector:
            matchLabels:
              k8s-app: kube-dns
      ports:
        - protocol: UDP
          port: 53
        - protocol: TCP
          port: 53
    # Allow Ollama host only (adjust IP/subnet to match your setup)
    - to:
        - ipBlock:
            cidr: 192.168.1.100/32  # Your Ollama host IP
      ports:
        - protocol: TCP
          port: 11434
```

Adjust the `namespace`, pod labels, and `cidr` to match your deployment. Use `/32` for a single IP or a broader subnet like `192.168.1.0/24` if needed.

### Docker Compose - Firewall Rules

For Docker deployments where Ollama runs on a separate machine, use host firewall rules:

```bash
# Get the hone container's IP
docker inspect hone | grep IPAddress

# Block all outbound except Ollama (replace IPs as needed)
iptables -I DOCKER-USER -s 172.17.0.2 -j DROP
iptables -I DOCKER-USER -s 172.17.0.2 -d 192.168.1.100 -p tcp --dport 11434 -j ACCEPT
```

For setups where Ollama is a sidecar container on the same host:

```yaml
services:
  hone:
    image: ghcr.io/heskew/hone-money:latest
    networks:
      - hone-internal

  ollama:
    image: ollama/ollama
    networks:
      - hone-internal

networks:
  hone-internal:
    internal: true  # No internet access - containers can only reach each other
```

## Operations

### Update

```bash
docker compose pull
docker compose up -d
```

### View Logs

```bash
docker compose logs -f hone
```

### Debug Logging

Enable debug logging with the `RUST_LOG` environment variable:

```bash
# In docker-compose.yml
environment:
  - RUST_LOG=hone_server=debug

# Or for k3s deployment
env:
  - name: RUST_LOG
    value: "hone_server=debug"
```

This shows detailed logs for authentication, trusted network checks, and API requests.

### Backup

```bash
docker compose exec hone /app/hone backup create
```

### Stop

```bash
docker compose down
```

## Mac Training Setup

Run model fine-tuning on a Mac while Hone runs on another machine (e.g., Raspberry Pi).

### Architecture

```
┌─────────────────┐         ┌─────────────────┐
│   Mac Studio    │         │   Pi (Docker)   │
│                 │  HTTP   │                 │
│  train.sh  ─────┼────────►│  hone-server    │
│  mlx-lm         │  API    │  SQLite DB      │
│  ollama         │         │                 │
└─────────────────┘         └─────────────────┘
```

### Prerequisites

On the Mac:
```bash
# MLX for Apple Silicon fine-tuning
pip install mlx-lm

# Ollama for model serving
brew install ollama
```

### Setup

1. **Generate an API key** on the Pi:
   ```bash
   openssl rand -hex 32
   # Output: e.g., a1b2c3d4e5f6...
   ```

2. **Add to Pi's `.env`**:
   ```
   HONE_API_KEYS=a1b2c3d4e5f6...
   ```

3. **Restart Hone** on the Pi:
   ```bash
   docker compose up -d
   ```

4. **Configure the Mac**:
   ```bash
   # Add to ~/.zshrc or ~/.bashrc
   export HONE_API_URL="http://pi-hostname:3000"
   export HONE_API_KEY="a1b2c3d4e5f6..."
   ```

### Usage

```bash
# List available training tasks
./scripts/train.sh --list

# Train a model
./scripts/train.sh --task classify_merchant --branch main

# Train with specific base model
./scripts/train.sh --task normalize_merchant --base-model gemma3:27b
```

The script:
1. Fetches training data from the Pi via API
2. Runs MLX LoRA fine-tuning locally
3. Creates an Ollama model from the adapter

### Network Options

The Mac needs to reach the Pi's port 3000. Options:

1. **Same LAN** - Use Pi's local IP (e.g., `http://192.168.1.x:3000`)
2. **Tailscale** - Use Pi's Tailscale IP (e.g., `http://100.x.x.x:3000`)
3. **SSH Tunnel** - `ssh -L 3000:localhost:3000 pi` then use `http://localhost:3000`
