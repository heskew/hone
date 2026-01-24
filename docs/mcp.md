---
title: MCP Server
description: Connect LLMs to your financial data via MCP
date: 2026-01-24
---

The MCP (Model Context Protocol) server exposes Hone's financial data to LLMs for conversational queries. All communication stays on your local network — no cloud services required.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Local Network                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────┐         ┌──────────────────────────┐   │
│  │  Raspberry Pi   │   HTTP  │   Mac / Windows          │   │
│  │  (hone serve)   │◄───────►│   with GPU               │   │
│  │                 │         │                          │   │
│  │  Port 3000: API │         │   Ollama / vLLM          │   │
│  │  Port 3001: MCP │         │   + MCP Client           │   │
│  └─────────────────┘         └──────────────────────────┘   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Key benefits:**
- **Data stays local** — All communication on local network
- **LLM-agnostic** — Works with Claude Desktop, Ollama-based agents, or custom clients
- **Read-only** — MCP tools only query data, never modify
- **Separation of concerns** — Pi handles data storage, Mac handles LLM inference

## Quick Start

### 1. Start Hone with MCP enabled

On your Raspberry Pi (or wherever Hone runs):

```bash
# Start with MCP on port 3001
hone serve --port 3000 --mcp-port 3001 --host 0.0.0.0

# Or with all your usual options
hone serve \
  --port 3000 \
  --mcp-port 3001 \
  --host 0.0.0.0 \
  --static-dir /path/to/ui/dist
```

You should see:
```
🚀 Starting Hone web server...
   Database: hone.db
   Listening: http://0.0.0.0:3000
   MCP server: http://0.0.0.0:3001/mcp
```

### 2. Connect an MCP Client

The MCP server exposes a JSON-RPC endpoint. You can call it directly from any HTTP client or custom agent:

```bash
# List available tools
curl http://pi-hostname:3001/mcp -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'

# Call a tool
curl http://pi-hostname:3001/mcp -X POST \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc":"2.0",
    "id":2,
    "method":"tools/call",
    "params":{
      "name":"get_spending_summary",
      "arguments":{"period":"this-month"}
    }
  }'
```

## Available Tools

| Tool | Description | Key Parameters |
|------|-------------|----------------|
| `search_transactions` | Find transactions | `query`, `tag`, `period`, `min_amount`, `max_amount` |
| `get_spending_summary` | Spending by category | `period` |
| `get_subscriptions` | List subscriptions | `status` (active/cancelled/excluded/all) |
| `get_alerts` | Waste detection alerts | `alert_type`, `include_dismissed` |
| `compare_spending` | Period comparison | `current_period`, `baseline_period` |
| `get_merchants` | Top merchants | `period`, `category`, `limit` |
| `get_account_summary` | Account overview | — |

### Period Presets

All period parameters accept:
- `this-month`, `last-month`
- `this-year`, `last-year`, `ytd`
- `last-30-days`, `last-90-days`, `last-12-months`
- `all`
- Custom date: `2024-01-15`

## Example Conversations

Once connected, you can ask questions naturally:

### Basic Queries

> "What did I spend on groceries last month?"

The LLM calls `get_spending_summary` with `period: "last-month"` and filters for the Groceries category.

> "Show me my Amazon purchases this year"

Calls `search_transactions` with `query: "Amazon"` and `period: "this-year"`.

### Subscription Management

> "What subscriptions am I paying for?"

Calls `get_subscriptions` with `status: "active"`.

> "Do I have any zombie subscriptions?"

Calls `get_alerts` with `alert_type: "zombie"`.

### Spending Analysis

> "How does my spending this month compare to last month?"

Calls `compare_spending` with default periods.

> "Where am I spending the most money?"

Calls `get_merchants` with `period: "this-year"`.

### Follow-up Questions

The LLM maintains context, so you can ask follow-ups:

> "What about just dining?"

If the previous query was about spending, it calls the same tool filtered to "Dining".

> "Show me those transactions"

Calls `search_transactions` to drill into details.

## Homelab Setup Example

Here's a complete homelab configuration:

### Network Setup

```
Router (192.168.1.1)
├── Raspberry Pi 4 (192.168.1.50)
│   └── Hone: ports 3000 (API) + 3001 (MCP)
└── Mac Studio (192.168.1.100)
    └── Ollama: port 11434
```

### Pi Configuration

`/etc/systemd/system/hone.service`:
```ini
[Unit]
Description=Hone Personal Finance
After=network.target

[Service]
Type=simple
User=pi
WorkingDirectory=/home/pi/hone
Environment=HONE_DB_KEY=your-encryption-key
Environment=OLLAMA_HOST=http://192.168.1.100:11434
Environment=OLLAMA_MODEL=llama3.2
ExecStart=/home/pi/hone/hone serve \
  --port 3000 \
  --mcp-port 3001 \
  --host 0.0.0.0 \
  --static-dir /home/pi/hone/ui/dist
Restart=always

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable hone
sudo systemctl start hone
```

Once running, query the MCP endpoint from any HTTP client on your network:

```bash
curl http://192.168.1.50:3001/mcp -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_spending_summary","arguments":{"period":"this-month"}}}'
```

## Security Considerations

### Network Isolation

The MCP server binds to the host you specify:
- `--host 127.0.0.1` — Only local connections (default)
- `--host 0.0.0.0` — All network interfaces (for LAN access)

For homelab use, `0.0.0.0` is fine since your network is trusted. Do NOT expose to the internet.

### Authentication

The MCP server currently shares the same authentication config as the main API:
- If `--no-auth` is set, MCP is unauthenticated (fine for home network)
- If auth is enabled, MCP requests need the same credentials

For home use with a private network, `--no-auth` is reasonable. The MCP tools are read-only — they can't modify your data.

### Firewall

If you have a firewall on the Pi:

```bash
# Allow MCP port from local network only
sudo ufw allow from 192.168.1.0/24 to any port 3001
```

## Troubleshooting

### "Connection refused"

1. Check Hone is running: `systemctl status hone`
2. Check it's listening on all interfaces: `ss -tlnp | grep 3001`
3. Check firewall: `sudo ufw status`

### "No tools available" in Claude Desktop

1. Check the MCP endpoint is reachable: `curl http://pi:3001/mcp`
2. Verify config file syntax (JSON must be valid)
3. Restart Claude Desktop completely

### Tools return empty results

1. Make sure you have data imported
2. Check the time period — default is "this-month"
3. Try `period: "all"` to see all data

### Slow responses

MCP tools query the database directly — they should be fast (<100ms). If slow:
1. Check Pi CPU/memory: `htop`
2. Check database size: `ls -lh hone.db`
3. Consider adding indexes (rare)

## Advanced: Building a Custom Agent

If you want to build your own conversational agent using Ollama + Hone MCP:

```python
import httpx
import json

MCP_URL = "http://192.168.1.50:3001/mcp"

def call_tool(name: str, args: dict) -> dict:
    """Call an MCP tool and return the result."""
    response = httpx.post(MCP_URL, json={
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": args
        }
    })
    return response.json()

# Example: Get spending summary
result = call_tool("get_spending_summary", {"period": "this-month"})
print(json.dumps(result, indent=2))
```

Integrate this with your Ollama agent's tool-calling capability for a fully local conversational finance assistant.

## What's Next

The MCP server provides read-only access. Future enhancements could include:
- Write tools (mark subscription as cancelled, dismiss alert)
- Streaming for large result sets
- Authentication tokens for MCP specifically
- WebSocket transport for real-time updates
