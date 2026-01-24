---
title: Ollama Integration
description: AI setup and configuration for Hone
date: 2026-01-24
---

Hone uses a pluggable AI backend system for local LLM inference. **Ollama is the primary supported backend** - this document covers its setup and usage.

## Two AI Modes

Hone supports two complementary AI modes:

1. **Classification Mode** (via `OLLAMA_HOST`) - Standard Ollama `/api/generate` endpoint for:
   - Merchant classification (tagging)
   - Merchant name normalization
   - Subscription detection
   - Receipt-to-transaction matching

2. **Agentic Mode** (via `ANTHROPIC_COMPATIBLE_HOST`) - Ollama's Anthropic Messages API for:
   - Tool-calling workflows where the AI can query your financial data
   - Deeper spending anomaly analysis
   - Richer duplicate subscription explanations
   - **Explore Mode** - conversational interface for querying financial data
   - Any task requiring iterative reasoning with data access

Both modes are optional and can be used independently or together.

### Other Backends

Other backends are supported through the same `AIBackend` trait:
- **OpenAI-compatible** - Works with Docker Model Runner, vLLM, LocalAI, llama-server, and any OpenAI-compatible endpoint
- **Mock** - For testing

All backends share the same prompts and response types. See "Alternative: OpenAI-Compatible Backend" below for non-Ollama setup.

AI features are optional - Hone works without them, falling back to pattern matching and the "Other" category.

## How It Works

During import, transactions are processed in two ways:

### Tagging
Transactions are tagged in this order:
1. **User rules** - Exact/contains/regex patterns you define
2. **Auto-patterns** - Built-in patterns on root tags (e.g., "WALMART" → Groceries)
3. **Bank category** - Uses bank-provided category if available (e.g., Amex "Transportation-Fuel" → Transport)
4. **Ollama** - LLM classifies the merchant name
5. **Fallback** - Unclassified transactions tagged as "Other"

Bank categories are mapped to Hone tags with 0.75 confidence. Supported mappings include:
- `Transportation-Fuel` → Transport > Gas
- `Transportation-Parking` → Transport > Parking
- `Transportation-Tolls` → Transport > Tolls
- `Transportation-Auto Services` → Transport > Auto
- `Transportation-*` → Transport (generic)
- `Restaurant-*` → Dining
- `*-Groceries` → Groceries
- `Entertainment-Associations` → Personal > Fitness (sports clubs, gyms)
- `Entertainment-*` → Entertainment (other)
- `Airlines-*`, `Lodging-*` → Travel
- `Healthcare-*`, `Pharmacy-*` → Healthcare
- `Fees & Adjustments-*`, `Financial-*` → Financial

Generic categories like "Merchandise & Supplies-Internet Purchase" fall through to Ollama for merchant-based classification.

### Merchant Normalization
After tagging, Ollama normalizes raw bank descriptions into clean merchant names:
- `AMZN MKTP US*2X7Y9Z` → `Amazon Marketplace`
- `TRADER JOE'S #456` → `Trader Joe's`
- `SPOTIFY USA*1234567` → `Spotify`

Normalized names are stored in the `merchant_normalized` column and used for display, while the original description is preserved for matching.

### Subscription Classification
During subscription detection, Ollama classifies merchants as subscription services or retail:
- **SUBSCRIPTION**: Netflix, Spotify, gym memberships, meal kits (HelloFresh, Blue Apron)
- **RETAIL**: Grocery stores (Trader Joe's, Costco), gas stations, restaurants

This reduces false positives where regular shopping gets flagged as a "zombie subscription". Classifications are cached in the `merchant_subscription_cache` table. User exclusions (via "Not a subscription" button) override Ollama classifications.

### Receipt-to-Transaction Matching
When matching receipts to bank transactions, some matches are "ambiguous" (score 0.5-0.85). For these, Ollama evaluates whether the receipt and transaction are likely the same purchase:

- **Tips**: Receipt shows $50, bank shows $60 → Ollama recognizes this as a 20% tip
- **Processing delays**: Receipt dated Jan 10, transaction dated Jan 12 → normal for credit cards
- **Merchant name variations**: Receipt says "Amazon", bank shows "AMZN MKTP US" → same merchant

The final match score combines algorithmic matching (60%) with Ollama's confidence (40%). Ollama's reasoning is stored in the match factors for transparency in the UI.

### Duplicate Detection Reasoning
When duplicate subscriptions are detected (e.g., 3 streaming services), Ollama analyzes what they have in common and what makes each unique:

**Example output:**
> **What they have in common:** All offer on-demand streaming of movies and TV series
>
> **What makes each unique:**
> - **Netflix:** International content, mature themes, extensive originals
> - **Disney+:** Family content, Marvel, Star Wars, Pixar exclusives
> - **HBO Max:** HBO originals, Warner Bros theatrical releases

This helps you understand whether you truly have redundant services or if each serves a distinct purpose. The analysis is stored in the alert and displayed in the alert detail modal.

Only service names and categories are sent to Ollama (no account info), preserving privacy.

## Setup

### Option 1: Remote Ollama Server (Recommended)

Run Ollama on a more powerful machine (e.g., Windows PC with NVIDIA GPU) and point Hone to it:

```bash
# On your Windows machine with GPU:
# 1. Install Ollama from https://ollama.com/download
# 2. Pull a model: ollama pull llama3.2
# 3. Ollama listens on localhost:11434 by default

# To expose to network, set environment variable before starting Ollama:
# OLLAMA_HOST=0.0.0.0:11434

# In Hone's .env on the Pi:
OLLAMA_HOST=http://192.168.1.100:11434  # Your Windows machine's IP
```

This gives you fast GPU-accelerated inference while keeping Hone on the Pi.

### Option 2: Ollama on the Same Machine

If running Hone directly on a machine (not in Docker), you can run Ollama locally:

```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Pull a model
ollama pull llama3.2

# Ollama runs on port 11434 by default
```

Set environment variables:
```bash
export OLLAMA_HOST=http://localhost:11434
export OLLAMA_MODEL=llama3.2  # optional, defaults to llama3.2
```

## Model Recommendations

### Text Models (Classification, Normalization, Entity Suggestions)

| Model | VRAM | Speed | Quality | Notes |
|-------|------|-------|---------|-------|
| `gemma3` | ~4GB | Fast | Excellent | **Recommended** - best instruction following |
| `llama3.2:3b` | ~3GB | Fast | Good | Alternative option |
| `llama3.2:1b` | ~2GB | Very fast | Decent | Fastest option |

**Why gemma3?** It excels at following formatting instructions precisely, which is critical for merchant normalization (preserving apostrophes in brand names like "Trader Joe's", "McDonald's").

### Vision Models (Receipt Parsing)

| Model | VRAM | Speed | Quality | Notes |
|-------|------|-------|---------|-------|
| `llama3.2-vision:11b` | ~8GB | Medium | Good | **Recommended** for receipts |
| `llava:13b` | ~10GB | Medium | Good | Alternative vision model |
| `llava:7b` | ~5GB | Fast | Decent | Smaller/faster option |

### Quick Start

```bash
# On your Windows machine with GPU:

# Required: text model for classification and normalization
ollama pull gemma3

# Required: vision model for receipt parsing
ollama pull llama3.2-vision:11b
```

### Environment Variables

```bash
# In your .env or shell
export OLLAMA_HOST=http://<windows-ip>:11434
export OLLAMA_MODEL=gemma3                     # Default text model
export OLLAMA_VISION_MODEL=llama3.2-vision:11b  # Vision model for receipts
```

**With NVIDIA GPU**: Use the recommended models above. GPU inference is ~10-50x faster than CPU.

**On Raspberry Pi** (CPU only): Stick with `llama3.2:1b` for text, skip vision (too slow).

## Agentic Mode Setup

Ollama 0.14+ supports the Anthropic Messages API, enabling tool-calling workflows. This lets the AI dynamically query your transaction data for richer analysis.

### Why Use Agentic Mode?

| Feature | Classification Only | With Agentic Mode |
|---------|--------------------|--------------------|
| Spending anomalies | Pre-assembled context, single prompt | AI investigates transactions, finds patterns |
| Duplicate analysis | Lists services with overlap | AI checks usage frequency, suggests which to cancel |
| Explanations | Based on provided data only | Can query for additional context |

### Configuration

```bash
# In your .env or shell (in addition to OLLAMA_HOST)
export ANTHROPIC_COMPATIBLE_HOST=http://<ollama-ip>:11434
export ANTHROPIC_COMPATIBLE_MODEL=llama3.1  # Must support tool calling
```

**Recommended models for agentic tasks:**
- `llama3.3` - **Recommended** - best instruction following, handles empty data gracefully
- `llama3.1` - Good alternative, faster than 3.3 but less precise with complex prompts
- `llama3.1:70b` or `llama3.3:70b` - Better reasoning, requires more VRAM
- `qwen3-coder` - Works but may require XML fallback parsing
- Any model with 32K+ context that supports function calling

**Performance note:** Agentic queries typically take 15-30+ seconds due to the multi-turn workflow (initial LLM call → tool execution → response processing). Larger models like llama3.3 produce better responses but are slower. This is a trade-off between quality and speed.

### How It Works

When agentic mode is enabled, certain analysis tasks use a multi-turn workflow:

1. Hone sends a prompt with available tools (search_transactions, get_merchants, etc.)
2. The AI decides what data it needs and calls tools
3. Hone executes the tools and returns results
4. The AI reasons about the data and may call more tools
5. When done, the AI returns a final explanation

**Example: Spending Anomaly Analysis**

```
AI: "I'll check the transactions for this category..."
    [calls get_merchants tool]
AI: "I see 3 new merchants this month. Let me look at the largest transactions..."
    [calls search_transactions tool]
AI: "The increase is due to:
    1. New gym membership at Equinox ($150/month)
    2. 4 DoorDash orders totaling $180 (vs $50 last month)
    3. One-time purchase at REI for $320"
```

### Tools Available to the AI

| Tool | Description |
|------|-------------|
| `search_transactions` | Search by query, date range, tag, or amount |
| `get_spending_summary` | Spending breakdown by category |
| `get_subscriptions` | List subscriptions with status and amounts |
| `get_alerts` | Active waste detection alerts |
| `compare_spending` | Compare spending between periods |
| `get_merchants` | Top merchants by spending amount |
| `get_account_summary` | Overview of all accounts |

All tool calls stay local—data never leaves your network.

### CLI Output

When running detection, you'll see which AI capabilities are active:

```bash
$ hone detect
🔍 Running waste detection...
   🤖 AI backend enabled (full: classification + agentic analysis)
   Mode: All detection types
```

Possible modes:
- `full: classification + agentic analysis` - Both backends configured
- `agentic analysis only` - Only orchestrator configured
- `classification only` - Only Ollama generate API configured
- Tips shown if neither is configured

### Explore Mode

Explore Mode provides a conversational interface for querying your financial data. Navigate to the Explore page and ask questions like:

- "What did I spend last month?"
- "Show my subscriptions"
- "Any zombie subscriptions?"
- "Compare this month to last month"

**Features:**
- Multi-turn conversations with session persistence (30-minute timeout)
- Model selector to switch between available Ollama models at runtime
- All queries tracked in AI Metrics as `explore_query` operations
- Tool call tracking: view which tools were called, their inputs, and outputs in AI Metrics detail view

The AI uses the same tools listed above to answer your questions, dynamically querying your data as needed.

## Testing the Connection

### Using the CLI

```bash
# Test Ollama connection and run sample classifications
hone ollama test

# Test with a specific merchant
hone ollama test --merchant "TARGET #1234 AUSTIN TX"

# Test receipt parsing with an image
hone ollama test --receipt /path/to/receipt.jpg
```

### Manual Testing

```bash
# Check Ollama is running
curl http://localhost:11434/api/tags

# Test a classification (what Hone does internally)
curl http://localhost:11434/api/generate -d '{
  "model": "llama3.2:3b",
  "prompt": "Classify this merchant name and return JSON only:\nMerchant: \"NETFLIX.COM*1234\"\n\nReturn format:\n{\"merchant\": \"normalized name\", \"category\": \"category\"}\n\nCategories: streaming, music, cloud_storage, fitness, news, food_delivery, shopping, utilities, other",
  "stream": false
}'
```

## Category Mapping

Ollama returns categories that Hone maps to your tag hierarchy:

| Ollama Category | Hone Tag |
|-----------------|----------|
| streaming, music, cloud_storage, news | Subscriptions |
| food_delivery, restaurant, dining | Dining |
| groceries | Groceries |
| shopping | Shopping |
| utilities | Utilities |
| transport, gas, rideshare | Transport |
| entertainment | Entertainment |
| travel, hotel, airline | Travel |
| healthcare, pharmacy | Healthcare |
| income, salary, deposit | Income |
| housing, rent, mortgage | Housing |
| gifts | Gifts |
| financial, bank, investment | Financial |
| fitness | Personal |
| other | Other |

## Disabling Ollama

Simply don't set `OLLAMA_HOST`. Hone will skip LLM classification and rely on rules and patterns.

## Performance Notes

- First request to Ollama loads the model into memory (~10-30 seconds)
- Subsequent requests are fast (~1-3 seconds per merchant on Pi)
- Ollama caches models in memory, so keep it running for best performance
- On resource-constrained systems, imports may be slower with Ollama enabled

## Troubleshooting

**Ollama not responding:**
```bash
# Check if running
curl http://localhost:11434/api/tags

# Check logs
docker logs ollama  # if using Docker
journalctl -u ollama  # if using systemd
```

**Out of memory on Pi:**
- Use a smaller model (`llama3.2:1b`)
- Increase swap space
- Run Ollama on a separate machine

**Slow classification:**
- Normal on first request (model loading)
- Consider `llama3.2:1b` for faster responses
- Check Pi temperature (may throttle if hot)

**Wrong classifications:**
- Add user rules for frequently misclassified merchants
- Rules take priority over Ollama

## Alternative: OpenAI-Compatible Backend

If you prefer to use Docker Model Runner, vLLM, LocalAI, llama-server, or another OpenAI-compatible server instead of Ollama, Hone supports this via the `openai_compatible` backend.

### When to Use

- **Docker Model Runner**: Runs in Docker Desktop, easy setup on Windows/Mac
- **vLLM**: High-performance inference server, great for GPUs
- **LocalAI**: Self-hosted, supports GGUF models
- **llama-server (llama.cpp)**: Lightweight server for llama.cpp models

All of these expose an OpenAI-compatible `/v1/chat/completions` endpoint.

### Configuration

```bash
# Select the OpenAI-compatible backend
export AI_BACKEND=openai_compatible  # or: vllm, localai, llamacpp (all aliases)

# Required: server URL
export OPENAI_COMPATIBLE_HOST="http://192.168.1.100:8080"

# Optional: model name (default: gpt-3.5-turbo)
export OPENAI_COMPATIBLE_MODEL="llama3.2"

# Optional: API key if your server requires authentication
export OPENAI_COMPATIBLE_API_KEY="your-api-key"
```

### Example: Docker Model Runner

```bash
# On your Windows/Mac machine with Docker Desktop:
# 1. Enable Docker Model Runner in Docker Desktop settings
# 2. Pull a model: docker model pull llama3.2
# 3. It runs on port 8080 by default

# In Hone's .env on the Pi:
AI_BACKEND=openai_compatible
OPENAI_COMPATIBLE_HOST=http://192.168.1.100:8080
OPENAI_COMPATIBLE_MODEL=llama3.2
```

### Example: vLLM

```bash
# On your GPU server:
# pip install vllm
# vllm serve meta-llama/Llama-3.2-3B-Instruct --port 8000

# In Hone's .env:
AI_BACKEND=vllm
OPENAI_COMPATIBLE_HOST=http://gpu-server:8000
OPENAI_COMPATIBLE_MODEL=meta-llama/Llama-3.2-3B-Instruct
```

### Example: LocalAI

```bash
# On your server:
# docker run -p 8080:8080 localai/localai:latest

# In Hone's .env:
AI_BACKEND=localai
OPENAI_COMPATIBLE_HOST=http://server:8080
OPENAI_COMPATIBLE_MODEL=gpt-3.5-turbo  # LocalAI's default model name
```

### Vision Models

For receipt parsing, you need a vision-capable model. Set it separately:

```bash
export OPENAI_COMPATIBLE_VISION_MODEL="llava:13b"  # or any vision model your server supports
```

Note: Not all OpenAI-compatible servers support vision. Check your server's documentation.

### Testing the Connection

```bash
# Check if server is running
curl http://your-server:8080/v1/models

# Test a completion
curl http://your-server:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3.2",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```
