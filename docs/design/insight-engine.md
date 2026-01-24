---
title: Insight Engine
description: Proactive financial insights design
date: 2026-01-24
---

## Overview

The Insight Engine is a pluggable system that proactively surfaces financial insights. Instead of waiting for users to ask the right questions, it continuously analyzes spending data and surfaces what's interesting, actionable, or concerning.

## Goals

1. **Understand** — Help users grasp *why* their finances look the way they do
2. **Refine** — Surface actionable opportunities to optimize spending
3. **Anticipate** — Forecast upcoming expenses before they surprise you
4. **Evolve** — Easy to add new insight types without touching core code

## Core Insight Types

### 1. Price Comparison

Compare item prices across merchants to find where you're overpaying.

**Requires:** Item-level receipt data (splits with descriptions)

**Example output:**
> "You paid $6.49 for milk at Whole Foods on Jan 15. The same item was $3.99 at Costco last week—48% more expensive."

**Analysis approach:**
- Group splits by normalized item description
- Match similar items across merchants (fuzzy matching + Ollama)
- Calculate price differences, highlight significant gaps
- Track price trends per item over time

### 2. Spending Explainer

Automatically explain *why* spending changed, without being asked.

**Requires:** Transaction history, category tags, merchant data

**Example output:**
> "You spent $412 more this month than your 3-month average. Here's why:
> - DoorDash: +$180 (12 orders vs usual 4—busy month at work?)
> - Amazon: +$156 (one-time: new headphones)
> - Gas: +$76 (prices up 8% this month)"

**Analysis approach:**
- Compare current period to rolling baseline
- Decompose variance by category and merchant
- Distinguish recurring increases from one-time spikes
- Use LLM to generate natural narrative

### 3. Expense Forecaster

Predict upcoming expenses based on detected patterns.

**Requires:** Subscription data, transaction history with dates

**Example output:**
> "Upcoming in the next 30 days:
> - Car insurance: $620 (due ~Jan 28, based on last 3 payments)
> - Netflix: $22.99 (due ~Feb 1)
> - Estimated groceries: ~$650 (based on your weekly average)"

**Analysis approach:**
- Use subscription detection data for known recurring charges
- Detect non-monthly patterns (quarterly, annual, semi-annual)
- Project variable categories using rolling averages
- Flag large upcoming expenses prominently

### 4. Category Drift

Detect gradual shifts in spending patterns that might go unnoticed.

**Requires:** 6+ months of transaction history

**Example output:**
> "Your dining spending has increased 15% each month for the past 4 months. You're now spending $840/month vs $520 six months ago."

**Analysis approach:**
- Calculate month-over-month change per category
- Detect sustained trends (not just noise)
- Distinguish inflation from behavior change
- Surface before it becomes a problem

### 5. Merchant Intelligence

Surface interesting patterns about specific merchants.

**Requires:** Transaction history, receipt data (when available)

**Example output:**
> "You shop at Target 3.2x per week on average. Your typical basket is $47. This month you've already spent $580 there—on track for $720 (vs $520 usual)."

**Analysis approach:**
- Frequency and spend per merchant
- Basket size trends
- Time-of-week/month patterns
- Cross-merchant substitution detection

### 6. Savings Opportunity Scanner

Proactively find ways to reduce spending.

**Requires:** Subscription data, transaction patterns

**Example output:**
> "Potential savings identified:
> - Hulu: No activity in 67 days, still charging $17.99/month
> - Spotify + Apple Music: You're paying for both ($26/month)
> - Annual vs monthly: Switching Netflix to annual would save $24/year"

**Analysis approach:**
- Leverage existing detection (zombies, duplicates)
- Add usage pattern analysis when possible
- Calculate annual vs monthly arbitrage
- Rank by impact

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                       Insight Engine                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌───────────┐  │
│  │   Price     │ │  Spending   │ │   Expense   │ │ Category  │  │
│  │ Comparison  │ │  Explainer  │ │ Forecaster  │ │   Drift   │  │
│  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘ └─────┬─────┘  │
│         │               │               │              │         │
│  ┌──────┴───────────────┴───────────────┴──────────────┴──────┐ │
│  │                    Insight Trait                            │ │
│  │  - analyze(context) → Vec<Finding>                         │ │
│  │  - relevance(finding, user_context) → f32                  │ │
│  │  - render(finding) → InsightCard                           │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    Insight Ranker                           │ │
│  │  - Deduplicate overlapping insights                         │ │
│  │  - Score by relevance × recency × actionability             │ │
│  │  - Apply user preferences (dismissed, snoozed)              │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                              │                                   │
└──────────────────────────────┼───────────────────────────────────┘
                               │
           ┌───────────────────┼───────────────────┐
           ▼                   ▼                   ▼
      Dashboard           Weekly Digest       Chat Query
       Widget               Email            "Why am I broke?"
```

## The Insight Trait

```rust
/// A type of financial insight that can be detected and surfaced
pub trait Insight: Send + Sync {
    /// Unique identifier for this insight type
    fn id(&self) -> &'static str;
    
    /// Human-readable name
    fn name(&self) -> &'static str;
    
    /// Analyze data and produce findings
    /// Called periodically or on-demand
    fn analyze(&self, ctx: &AnalysisContext) -> Result<Vec<Finding>>;
    
    /// Score how relevant a finding is right now (0.0 - 1.0)
    /// Used for ranking and filtering
    fn relevance(&self, finding: &Finding, user_ctx: &UserContext) -> f32;
    
    /// Generate a natural language explanation
    /// May use LLM for narrative generation
    fn explain(&self, finding: &Finding, ollama: &ModelRouter) -> Result<String>;
    
    /// Render as a UI-friendly structure
    fn render(&self, finding: &Finding) -> InsightCard;
}

/// Context provided to insight analyzers
pub struct AnalysisContext<'a> {
    pub transactions: &'a [Transaction],
    pub subscriptions: &'a [Subscription],
    pub splits: &'a [Split],
    pub receipts: &'a [Receipt],
    pub tags: &'a TagTree,
    pub date_range: DateRange,
    pub ollama: &'a ModelRouter,
}

/// A detected insight finding
pub struct Finding {
    pub insight_type: String,
    pub key: String,              // Deduplication key
    pub severity: Severity,       // Info, Attention, Warning, Alert
    pub data: serde_json::Value,  // Insight-specific payload
    pub detected_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// How a finding is presented in the UI
pub struct InsightCard {
    pub title: String,
    pub summary: String,           // One-line description
    pub detail: Option<String>,    // Expanded explanation
    pub actions: Vec<InsightAction>,
    pub visualization: Option<Visualization>,
    pub severity: Severity,
}

pub enum InsightAction {
    Dismiss,
    Snooze { days: u32 },
    ViewTransactions { filter: TransactionFilter },
    CancelSubscription { subscription_id: i64 },
    Custom { label: String, action: String },
}

pub enum Visualization {
    Trend { data: Vec<(String, f64)> },
    Comparison { items: Vec<ComparisonItem> },
    Breakdown { segments: Vec<(String, f64, String)> }, // label, value, color
    Timeline { events: Vec<TimelineEvent> },
}
```

## Insight Registry

```rust
pub struct InsightEngine {
    insights: Vec<Box<dyn Insight>>,
    store: InsightStore,  // Persistence for findings, dismissals, snoozes
}

impl InsightEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            insights: vec![],
            store: InsightStore::new(),
        };
        
        // Register built-in insights
        engine.register(Box::new(PriceComparisonInsight::new()));
        engine.register(Box::new(SpendingExplainerInsight::new()));
        engine.register(Box::new(ExpenseForecasterInsight::new()));
        engine.register(Box::new(CategoryDriftInsight::new()));
        engine.register(Box::new(MerchantIntelligenceInsight::new()));
        engine.register(Box::new(SavingsOpportunityInsight::new()));
        
        engine
    }
    
    pub fn register(&mut self, insight: Box<dyn Insight>) {
        self.insights.push(insight);
    }
    
    /// Run all insights and return ranked findings
    pub fn analyze(&self, ctx: &AnalysisContext) -> Result<Vec<RankedFinding>> {
        let mut all_findings = vec![];
        
        for insight in &self.insights {
            match insight.analyze(ctx) {
                Ok(findings) => all_findings.extend(findings),
                Err(e) => {
                    tracing::warn!(insight = insight.id(), error = %e, "Insight failed");
                }
            }
        }
        
        // Deduplicate, rank, filter
        let ranked = self.rank_findings(all_findings, &ctx.user_context);
        
        Ok(ranked)
    }
    
    /// Get top N insights for dashboard display
    pub fn top_insights(&self, ctx: &AnalysisContext, n: usize) -> Result<Vec<InsightCard>> {
        let ranked = self.analyze(ctx)?;
        
        ranked.into_iter()
            .take(n)
            .map(|f| self.render_finding(&f))
            .collect()
    }
}
```

## Persistence

Insights need to track:
- **Dismissed findings** — Don't resurface
- **Snoozed findings** — Resurface after delay
- **User feedback** — Was this helpful? (for learning)
- **Finding history** — When was this first/last detected?

```sql
CREATE TABLE insight_findings (
    id INTEGER PRIMARY KEY,
    insight_type TEXT NOT NULL,
    finding_key TEXT NOT NULL,           -- Deduplication
    severity TEXT NOT NULL,
    data JSON NOT NULL,
    first_detected_at DATETIME NOT NULL,
    last_detected_at DATETIME NOT NULL,
    status TEXT DEFAULT 'active',        -- active, dismissed, snoozed
    snoozed_until DATETIME,
    user_feedback TEXT,                  -- helpful, not_helpful, null
    UNIQUE(insight_type, finding_key)
);

CREATE INDEX idx_findings_status ON insight_findings(status, last_detected_at);
```

## API Endpoints

```
GET  /api/insights                    # Top insights for dashboard
GET  /api/insights/all                # All active insights (paginated)
GET  /api/insights/:type              # Insights of specific type
POST /api/insights/:id/dismiss        # Dismiss a finding
POST /api/insights/:id/snooze         # Snooze for N days
POST /api/insights/:id/feedback       # Mark helpful/not helpful
POST /api/insights/refresh            # Re-run analysis
```

## Dashboard Integration

The dashboard gets a new "Insights" section that surfaces the top 3-5 most relevant findings:

```tsx
function InsightsWidget() {
  const { data: insights } = useQuery(['insights'], fetchTopInsights);
  
  return (
    <div className="space-y-3">
      <h2 className="text-lg font-semibold">What's Going On</h2>
      {insights?.map(insight => (
        <InsightCard 
          key={insight.id}
          insight={insight}
          onDismiss={() => dismissInsight(insight.id)}
          onSnooze={(days) => snoozeInsight(insight.id, days)}
        />
      ))}
    </div>
  );
}
```

## Learning & Evolution

### Implicit Feedback

- **Dismissed immediately** → Probably not relevant
- **Clicked to expand** → Interesting
- **Took action** → Very valuable
- **Snoozed** → Maybe relevant, bad timing

### Relevance Tuning

Track which insight types get engagement and adjust ranking:

```rust
pub struct InsightMetrics {
    pub insight_type: String,
    pub shown_count: u32,
    pub dismissed_count: u32,
    pub expanded_count: u32,
    pub action_taken_count: u32,
    pub helpful_count: u32,
    pub not_helpful_count: u32,
}

impl InsightEngine {
    fn relevance_boost(&self, insight_type: &str) -> f32 {
        let metrics = self.store.get_metrics(insight_type);
        let engagement_rate = metrics.action_taken_count as f32 / metrics.shown_count as f32;
        
        // Boost high-engagement insight types
        1.0 + (engagement_rate * 0.5)
    }
}
```

## Adding a New Insight Type

1. Create a struct implementing `Insight` trait
2. Register it in `InsightEngine::new()`
3. Done—no changes to API, UI, or persistence

```rust
// Example: Detect when you're spending more at a merchant than usual
pub struct MerchantSpikeInsight;

impl Insight for MerchantSpikeInsight {
    fn id(&self) -> &'static str { "merchant_spike" }
    fn name(&self) -> &'static str { "Merchant Spending Spike" }
    
    fn analyze(&self, ctx: &AnalysisContext) -> Result<Vec<Finding>> {
        let mut findings = vec![];
        
        // Group by merchant, compare to baseline
        for (merchant, txns) in ctx.transactions.group_by_merchant() {
            let current = txns.current_period_total();
            let baseline = txns.baseline_average();
            
            if current > baseline * 1.5 && current - baseline > 50.0 {
                findings.push(Finding {
                    insight_type: self.id().to_string(),
                    key: format!("spike:{}", merchant),
                    severity: Severity::Attention,
                    data: json!({
                        "merchant": merchant,
                        "current": current,
                        "baseline": baseline,
                        "increase_pct": (current / baseline - 1.0) * 100.0,
                    }),
                    detected_at: Utc::now(),
                    expires_at: None,
                });
            }
        }
        
        Ok(findings)
    }
    
    // ... other trait methods
}
```

## Data Requirements

| Insight Type | Minimum Data | Ideal Data |
|--------------|--------------|------------|
| Price Comparison | Splits with descriptions | Receipts with item-level detail |
| Spending Explainer | 2+ months transactions | 6+ months, tagged |
| Expense Forecaster | 3+ months transactions | 12+ months, subscriptions detected |
| Category Drift | 6+ months transactions | 12+ months, tagged |
| Merchant Intelligence | 1+ month transactions | 6+ months |
| Savings Opportunity | Subscriptions detected | + usage data |

## Future: Conversational Interface

The Insight Engine can power a chat interface:

```
User: "Why did I spend so much this month?"

Engine: 
1. Detect intent → SpendingExplainer
2. Run analysis for current month
3. Generate narrative via LLM
4. Return with follow-up suggestions

"You spent $3,240 this month, which is $680 more than usual. 
The biggest factors were:
- A $450 car repair (one-time)
- DoorDash: $180 more than usual (11 extra orders)
- Target: $120 more than usual

Would you like me to dig into any of these?"
```

## Implementation Phases

### Phase 1: Foundation
- [ ] Insight trait and engine structure
- [ ] Persistence layer (findings table)
- [ ] API endpoints
- [ ] Dashboard widget (show top 3)

### Phase 2: Core Insights
- [ ] Spending Explainer (anomaly + narrative)
- [ ] Expense Forecaster (subscriptions + patterns)
- [ ] Savings Opportunity (wrap existing detection)

### Phase 3: Advanced Insights
- [ ] Price Comparison (requires receipt/split data)
- [ ] Category Drift (requires history)
- [ ] Merchant Intelligence

### Phase 4: Learning
- [ ] Feedback collection
- [ ] Engagement metrics
- [ ] Relevance tuning

### Phase 5: Conversational
- [ ] Intent detection
- [ ] Query → Insight routing
- [ ] Follow-up handling