---
title: Architecture
description: System vision and design layers
date: 2026-01-24
---

## Vision

Hone is the **personal financial brain for your household**—a local-first system that understands your spending, learns from your feedback, proactively surfaces what matters, and answers any question you think to ask.

## Core Principles

1. **Local data, local AI** — All data stays on your hardware. LLM inference via Ollama on your network. No cloud dependencies for core functionality.

2. **Raw data is sacred** — Original transaction data is never modified or discarded. Everything can be re-analyzed with better models and prompts.

3. **Learn and adapt** — The system gets smarter from your corrections, dismissals, and feedback. New models and prompts improve it without code changes.

4. **Proactive, not just reactive** — Surfaces insights before you ask. You shouldn't need to know what questions to ask.

5. **Fast for the common case** — Structured indexes enable instant lookups. LLM only when interpretation is needed.

6. **Evolvable** — Swap models, update prompts, add capabilities—without rewriting core code.

## Architecture Overview

```
+--[ INTERFACE LAYER ]--------------------------------------------------+
|                                                                        |
|   Dashboard          Chat              Scheduled Jobs                  |
|   Glanceable state   Any question      "What should I know?"           |
|                                        Daily/weekly                    |
+----------------------------------+-------------------------------------+
                                   |
                                   v
+--[ INTELLIGENCE LAYER ]-----------------------------------------------+
|                                                                        |
|   Context Assembler                                                    |
|   Given a question or task, retrieves relevant context:                |
|   - Transactions for the time period                                   |
|   - User's rules, patterns, preferences                                |
|   - Historical baselines and comparisons                               |
|   - Previous feedback on similar items                                 |
|                                  |                                     |
|                                  v                                     |
|   Prompt Library                                                       |
|   Versioned prompt templates stored as files                           |
|   Edit prompts without recompiling. A/B test versions.                 |
|                                  |                                     |
|                                  v                                     |
|   Model Router                                                         |
|   Config-driven task -> model mapping                                  |
|   Swap models via config. Track performance per task.                  |
|                                                                        |
+----------------------------------+-------------------------------------+
                                   |
                                   v
+--[ LEARNING LAYER ]---------------------------------------------------+
|                                                                        |
|   Everything here is derived and can be regenerated from raw data.     |
|                                                                        |
|   User Rules         Learned Patterns      LLM Cache                   |
|   Explicit:          Implicit:             Cached results:             |
|   "COSTCO ->         "User corrected       - Merchant -> category      |
|    Groceries"        X to Y"               - Is subscription?          |
|   Takes priority     Applied to new        Invalidate on               |
|   over everything    imports               model/prompt change         |
|                                                                        |
|   Feedback Store                                                       |
|   Captures how you interact with the system:                           |
|   - Dismissed alerts, corrections, exclusions                          |
|   - Helpful/not helpful ratings                                        |
|   Fed back into prompts as examples and constraints.                   |
|                                                                        |
+----------------------------------+-------------------------------------+
                                   |
                                   v
+--[ STRUCTURED INDEX ]-------------------------------------------------+
|                                                                        |
|   Normalized, indexed views for fast queries. Rebuildable from raw.    |
|                                                                        |
|   transactions     subscriptions     tags          entities            |
|   Normalized       Detected          Hierarchy     People, pets,       |
|   merchant names   patterns          + rules       vehicles            |
|                                                                        |
+----------------------------------+-------------------------------------+
                                   |
                                   v
+--[ IMMUTABLE FOUNDATION ]---------------------------------------------+
|                                                                        |
|   Raw Transaction Store                                                |
|   Every CSV row preserved exactly as imported.                         |
|   NEVER modified. Source of truth for re-analysis.                     |
|                                                                        |
|   Receipts Store           Encryption + Backup                         |
|   Original images          SQLCipher at rest                           |
|   preserved                Automated backups                           |
|                                                                        |
+-----------------------------------------------------------------------+
```

## Layer Details

### Immutable Foundation

**Purpose:** Store raw data that can always be re-processed.

**What's stored:**
- `raw_transactions` — Original CSV rows as JSON, never modified
- `receipts` — Original images, parsed data stored separately
- `import_sessions` — Metadata about each import

**Key invariant:** Nothing in this layer is ever updated or deleted (except by explicit user action). All analysis can be re-run against this data.

**Already implemented:** ✅ `original_data` and `import_format` columns exist. Receipts store original images.

---
### Structured Index
**Purpose:** Fast queries without LLM. Rebuildable from raw data.
**What's stored:**
- `transactions` — Normalized view with indexed fields
- `subscriptions` — Detected recurring patterns
- `tags` — Hierarchy and assignments
- `entities` — People, pets, vehicles, properties
- `splits` — Line items within transactions
- `locations` — Where purchases occurred
**Key property:** Can be fully rebuilt by re-processing raw data with current rules, patterns, and models.
**Already implemented:** ✅ Core tables exist. Add explicit "rebuild from raw" capability.
---
### Learning Layer

**Purpose:** Capture knowledge that improves over time.

#### User Rules (Explicit)
What you've told the system directly:
- Tag rules ("COSTCO" → Groceries)
- Merchant corrections ("This is Trader Joe's, not TRADER JOE")
- Exclusions ("This isn't a subscription")
- Entity assignments ("Gas purchases are for the Honda")

**Priority:** Highest. User rules override everything.

#### Learned Patterns (Implicit)
What the system infers from your behavior:
- Merchant → tag associations from manual tagging
- Merchant name variations from corrections
- Time-of-day patterns, frequency patterns

**Priority:** High. Applied to new data automatically.

#### LLM Cache
Cached results from LLM calls:
- Merchant classifications
- Subscription determinations
- Generated explanations

**Key property:** Invalidated when model or prompt version changes. Safe to rebuild.

#### Feedback Store
How you interact with the system:
- Dismissed alerts (with reason, timestamp)
- Snoozed items (resurface date)
- Helpful/not helpful ratings
- Corrections to LLM outputs

**Usage:** Fed into prompts as examples. "User previously dismissed similar alerts because..."

**Implemented:** ✅ `user_feedback` table with API endpoints and UI. Captures helpful/not_helpful ratings, corrections, dismissals with revert capability.

---
### Intelligence Layer
**Purpose:** Turn data + context into understanding.
#### Context Assembler
Given a question or task, retrieves everything the LLM needs:
```rust
pub struct ContextAssembler {
    db: Database,
    learning: LearningStore,
}
impl ContextAssembler {
    /// Assemble context for a spending explanation
    pub fn for_spending_explanation(&self, period: DateRange) -> Context {
        Context {
            transactions: self.db.transactions_in_range(period),
            baseline: self.db.average_spending(period.previous_n_months(3)),
            user_rules: self.learning.tag_rules(),
            previous_feedback: self.learning.feedback_for_category("spending_explanation"),
            subscriptions: self.db.active_subscriptions(),
        }
    }
    
    /// Assemble context for a natural language query
    pub fn for_query(&self, query: &str) -> Context {
        // Extract entities from query (dates, merchants, categories)
        // Pull relevant data
        // Include user's rules and preferences
        // Include previous similar queries and feedback
    }
}
```
**Implemented:** ✅ See `crates/hone-core/src/context.rs`.
#### Prompt Library
Prompts stored as files, not hardcoded in Rust:
```
prompts/
├── classify_merchant.v2.md
├── explain_spending.v1.md
├── find_subscriptions.v3.md
├── weekly_summary.v1.md
├── answer_query.v1.md
└── suggest_savings.v1.md
```
Each prompt file:
```markdown
---
id: explain_spending
version: 1
model_preference: reasoning
---

You are a financial analyst helping a family understand their spending.

The user wants to understand why spending changed.
## Their Rules
{{user_rules}}

## Previous Feedback
{{previous_feedback}}

## Current Period ({{period}})
{{current_spending}}
## Baseline (3-month average)
{{baseline_spending}}

## Notable Transactions
{{notable_transactions}}

Explain why spending changed. Be specific about which merchants and 
categories drove the change. Distinguish one-time expenses from trends.
Keep it concise—2-3 paragraphs max.
```

**Benefits:**
- Edit prompts without recompiling
- Version prompts, A/B test
- Include user feedback directly in prompts
- Non-developers can improve prompts

**Implemented:** ✅ Prompt library with override support in `prompts.rs`. CLI: `hone prompts list|show|path`.

#### Model Router

Config-driven model selection:

```toml
# config/models.toml
[routing]
fast_classification = "llama3.2:3b"
structured_extraction = "gemma3"
reasoning = "llama3.1:8b"
vision = "llama3.2-vision:11b"
[fallbacks]
reasoning = ["llama3.1:8b", "gemma3"]
```
**Implemented:** ✅ Model router in `model_router.rs` with task-based routing, health tracking, and config in `config/models.toml`.
---
### Interface Layer

**Purpose:** How you interact with the financial brain.

#### Dashboard

Glanceable state:
- Key metrics (spending this month, vs average)
- Active alerts requiring attention
- Recent insights surfaced by scheduled jobs
- Quick actions

**Already implemented:** ✅ Enhance with insight surfacing.

#### Chat

Natural language interface:

```
You: Why did we spend so much this month?

Hone: [Context Assembler pulls relevant data]
      [Prompt Library loads explain_spending.v1]
      [Model Router selects reasoning model]
      [LLM generates response]
      
      You spent $4,230 this month—$680 more than your 3-month 
      average. The main drivers:
      
      • Car repair at Firestone: $450 (one-time)
      • DoorDash: $285 vs $135 usual (+$150)
      • Target: 6 trips vs 3 usual
      
      The DoorDash increase was mostly in the second half of 
      the month—14 orders in two weeks.
```

**Key properties:**
- Context-aware (remembers conversation)
- Uses your rules and feedback
- Can trigger actions ("cancel that subscription")

**Not yet implemented:** 🚧 Build chat interface with context.

#### Scheduled Jobs

Periodic analysis that surfaces insights without being asked:

```rust
// Daily: Quick check
async fn daily_check(ctx: &AppContext) -> Vec<Insight> {
    let prompt = ctx.prompts.load("daily_check.v1");
    let context = ctx.assembler.for_daily_check();
    
    ctx.llm.generate(prompt, context).await
}

// Weekly: Deeper analysis
async fn weekly_summary(ctx: &AppContext) -> WeeklySummary {
    let prompt = ctx.prompts.load("weekly_summary.v1");
    let context = ctx.assembler.for_weekly_summary();
    
    ctx.llm.generate(prompt, context).await
}
```

What scheduled jobs ask:
- "Any new subscriptions I should know about?"
- "Anything unusual in the last 7 days?"
- "Any upcoming large expenses?"
- "Any savings opportunities?"

Results surface on dashboard or via notification.

**Not yet implemented:** 🚧 Build scheduler and insight surfacing.

---
## What Changes from Current Design
### Keep As-Is
| Component | Why |
|-----------|-----|
| Raw data preservation | Already stores `original_data` JSON |
| SQLCipher encryption | Security requirement met |
| Backup system | Works well |
| Tag hierarchy | Good structure |
| Learning caches | `merchant_name_cache`, `merchant_tag_cache` work |
| Model router concept | Design is sound |

### Simplify

| Current | Becomes |
|---------|---------|
| Hardcoded detection algorithms | Prompt templates + LLM |
| Rust-based intent parsing | Let the LLM parse intent |
| Multiple insight modules (code) | Prompt templates + scheduled jobs |
| Complex subscription detection | "Find recurring charges" prompt |
### Add
| Component | Purpose |
|-----------|---------|
| Prompt Library | File-based, versioned prompts |
| Context Assembler | Smart retrieval for LLM context |
| Chat Interface | Natural language queries |
| Scheduled Jobs | Proactive insight surfacing |
| Structured Feedback | Capture corrections and ratings |
---
## Re-analysis Capability

Critical requirement: Nothing lost in processing.

### How It Works

```rust
/// Rebuild all derived data from raw transactions
pub async fn rebuild_from_raw(ctx: &AppContext, options: RebuildOptions) -> Result<()> {
    // 1. Clear derived data (optional, based on options)
    if options.clear_index {
        ctx.db.clear_structured_index().await?;
    }
    if options.clear_cache {
        ctx.db.clear_llm_cache().await?;
    }
    
    // 2. Re-process each raw transaction
    let raw_txns = ctx.db.all_raw_transactions().await?;
    
    for raw in raw_txns {
        // Parse from original format
        let parsed = parse_raw_transaction(&raw)?;
        
        // Apply current rules and patterns
        let tagged = ctx.tagger.tag(&parsed).await?;
        
        // Run through current LLM (if cache cleared)
        let enriched = ctx.enricher.enrich(&tagged).await?;
        
        // Update structured index
        ctx.db.upsert_transaction(&enriched).await?;
    }
    
    // 3. Re-run detection
    ctx.detector.detect_all().await?;
    
    Ok(())
}
```

### When to Rebuild

- **New model:** Clear LLM cache, re-run classifications
- **New prompt:** Clear relevant cache entries, re-run
- **New rules:** Re-apply tagging (fast, no LLM needed)
- **Bug fix:** Full rebuild from raw

### CLI Support

```bash
# Re-tag all transactions with current rules (fast)
hone rebuild --tags-only

hone rebuild --reclassify

hone rebuild --full

hone rebuild --import-session 42
```

---
## Learning and Adaptation
### Explicit Learning (User Actions)
| Action | What's Learned | How It's Used |
|--------|----------------|---------------|
| Create tag rule | "COSTCO" → Groceries | Applied to all future imports |
| Correct merchant name | "TRADER JOE'S #123" → "Trader Joe's" | Cached, applied to similar |
| Correct category | Move transaction to different tag | Creates implicit pattern |
| "Not a subscription" | Exclude merchant from detection | Cached as explicit rule |
| Dismiss alert | Record reason | Avoid similar alerts |
| Rate explanation | Helpful/not helpful | Improve prompt with examples |

### Implicit Learning (Observed Behavior)

| Observation | What's Inferred | How It's Used |
|-------------|-----------------|---------------|
| User always tags X as Y | Merchant→tag pattern | Auto-apply to new imports |
| User ignores certain alerts | Low value alert type | Reduce prominence |
| User frequently asks about Z | Important category | Surface proactively |
| Time-of-month patterns | Predictable expenses | Better forecasting |
### Feedback in Prompts
Prompts include relevant feedback:
```markdown
## Previous Feedback

The user has indicated:
- "Not helpful" on explanations that are too vague
- Prefers specific merchant names over categories
- Dismissed similar "dining increased" alerts before—focus on actionable items
## Examples of Good Explanations
[Include examples user rated as helpful]
```

---

## Extensibility Points

### MCP (Model Context Protocol)

Potential uses:
- **Hone as MCP server:** Expose financial data to other tools (Claude Code, etc.)
- **External MCP servers:** Pull data from other sources (calendar for trip planning, email for receipts)

Not core architecture, but a clean interface layer could expose MCP endpoints.

### Skills / Agents

The prompt library is essentially a skill system:
- Each prompt is a specialized skill
- Scheduled jobs are autonomous agents
- Chat can dispatch to appropriate skill based on query
### Plugin System (Future)
If needed, the prompt library pattern extends to:
- Custom prompts for specific needs
- User-contributed prompt improvements
- Domain-specific analysis (freelancer income, rental properties, etc.)

---

## Implementation Roadmap

### Phase 1: Foundation (Now)

**Goal:** Ensure raw data is fully preserved and rebuildable.

- [ ] Audit `original_data` coverage—ensure all bank formats preserve everything
- [ ] Add `hone rebuild` CLI command
- [ ] Add rebuild API endpoint
- [ ] Test: import → rebuild → verify identical results
### Phase 2: Prompt Library ✅
**Goal:** Move prompts out of Rust code.
- [x] Create `prompts/` directory structure
- [x] Implement prompt loader with template variables
- [x] Migrate existing Ollama prompts to files
- [x] Add prompt versioning and metadata
- [x] Override support: user customizations in data dir, defaults embedded
### Phase 2.5: Model Router ✅
**Goal:** Task-based model routing with health tracking.
- [x] Define TaskType enum and RouterConfig struct
- [x] Implement ModelRouter with task→model lookup
- [x] Add health tracking (consecutive failures → fallback)
- [x] Config file support with override in data dir

### Phase 3: Context Assembler ✅

**Goal:** Smart context retrieval for LLM calls.

- [x] Define context types for each use case
- [x] Implement assembler with relevant data retrieval
- [x] Include user rules and feedback in context
- [ ] Test: same query with different context → different responses
### Phase 4: Structured Feedback ✅
**Goal:** Capture user feedback systematically.
- [x] Add feedback tables (ratings, corrections, dismissal reasons)
- [x] UI for rating explanations (helpful/not helpful)
- [x] Feedback history page with stats, filters, revert capability
- [ ] Capture implicit feedback (ignored suggestions)
- [x] Feed into prompt context
### Phase 5: Chat Interface (Week 4-6)
**Goal:** Natural language queries with context.
- [ ] Basic chat UI
- [ ] Conversation context tracking
- [ ] Query → context → prompt → response flow
- [ ] Follow-up handling

### Phase 6: Scheduled Jobs (Week 6-8)

**Goal:** Proactive insight surfacing.

- [ ] Scheduler infrastructure (cron or built-in)
- [ ] Daily quick check job
- [ ] Weekly summary job
- [ ] Surface results on dashboard
### Phase 7: Polish (Week 8+)
- [ ] A/B testing for prompts
- [ ] Model performance comparison
- [ ] Notification system for important insights
- [ ] Mobile-friendly chat

---

## Success Criteria

After 6 months, Hone should:

1. **Answer any question** about your finances in natural language
2. **Proactively surface** what you need to know without being asked
3. **Learn from corrections** and get smarter over time
4. **Swap models** by changing config, not code
5. **Improve prompts** by editing files, not deploying
6. **Re-analyze everything** when you upgrade models or fix bugs
7. **Never lose data** — raw transactions always available

---

## What NOT to Build

Explicit anti-goals to avoid scope creep:

- ❌ Real-time bank sync (CSV import is fine)
- ❌ Multi-user permissions (single household)
- ❌ Complex budgeting system (other tools do this)
- ❌ Investment tracking (different domain)
- ❌ Mobile app (responsive web is enough)
- ❌ Cloud sync (local-first is the point)
- ❌ Generative UI (standard components are enough)