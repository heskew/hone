---
title: Explore Mode
description: Conversational query interface design
date: 2026-01-24
---

## Overview

Explore Mode is a conversational interface for asking questions about your finances. Instead of navigating dashboards and drilling through reports, you ask in plain English and get answers with context.

It complements the Insight Engine:
- **Insight Engine** = Proactive ("here's what you should know")
- **Explore Mode** = Reactive ("answer my question")

## Architecture: MCP Server (Recommended)

The simplest path to Explore Mode: expose Hone data via MCP (Model Context Protocol), letting any LLM with tool support query your finances.

```
┌─────────────────────────────────────────────────────────────┐
│                    Local Network Only                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────┐         ┌──────────────────────────┐   │
│  │   Hone on Pi    │   HTTP  │   Mac / Windows          │   │
│  │  (hone serve)   │◄───────►│   LLM (Ollama/vLLM)     │   │
│  │                 │         │   + MCP Client           │   │
│  │  Port 3000: API │         │                          │   │
│  │  Port 3001: MCP │         │   Claude Desktop or      │   │
│  └─────────────────┘         │   custom agent           │   │
│                              └──────────────────────────┘   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Key benefits:**
- **Data stays local** — All communication on local network, no cloud services
- **LLM-agnostic** — Works with Claude Desktop, Ollama, vLLM, any MCP-capable client
- **Read-only** — MCP tools only query data, no modifications
- **Simple** — No custom intent parsing, conversation state, or UI needed

### MCP Tools

| Tool | Description |
|------|-------------|
| `search_transactions` | Find transactions by query, date, tag, amount |
| `get_spending_summary` | Spending by category for a period |
| `get_subscriptions` | Active/cancelled/all subscriptions |
| `get_alerts` | Zombie, price increase, duplicate alerts |
| `compare_spending` | Period-over-period comparison |
| `get_merchants` | Top merchants, spending per merchant |
| `get_account_summary` | Account balances and recent activity |

### Setup

```bash
# Enable MCP on Hone
hone serve --port 3000 --mcp-port 3001

# Or via environment
HONE_MCP_PORT=3001 hone serve
```

See [mcp.md](../mcp.md) for details on calling the MCP endpoint.

### Example Queries

With an MCP client connected, ask naturally:
- "What did I spend on coffee last month?"
- "Show me my biggest expenses this year"
- "Do I have any zombie subscriptions?"
- "Compare my dining spending this month vs last month"
- "Which streaming services am I paying for?"

The LLM interprets these and calls the appropriate tools.

---
## Alternative: Full Conversational System
For a richer in-app experience with follow-up context, goal flows, and custom UI, see the detailed design below.
## Goals
1. **Natural interaction** — Ask questions like you'd ask a person
2. **Contextual follow-ups** — "Why?" drills deeper, "What about X?" refocuses
3. **Goal-oriented flows** — "Help me save $200/month" guides you through options
4. **Fast answers** — Most queries answered in seconds, not waiting for generation
5. **Graceful escalation** — Simple queries = instant; complex = LLM-powered
## User Experience
### Basic Query
```
You: Where did my money go last month?

Hone: Last month you spent $4,230. Here's the breakdown:

       Housing      $1,850  ████████████████░░░░  44%
       Groceries      $620  █████░░░░░░░░░░░░░░░  15%
       Dining         $480  ████░░░░░░░░░░░░░░░░  11%
       Transport      $340  ███░░░░░░░░░░░░░░░░░   8%
       Shopping       $290  ██░░░░░░░░░░░░░░░░░░   7%
       Other          $650  █████░░░░░░░░░░░░░░░  15%
       Dining is up 34% from your average. Want me to dig into that?
```

### Follow-up Conversation

```
You: Yeah, what's up with dining?
Hone: Your dining spending was $480 last month vs $360 average.
       The increase is mostly DoorDash:
       • 14 orders last month (vs 6 typical)
       • $285 total (+$150 from usual)
       
       Looks like the second half of the month had a spike—
       9 orders in the last two weeks.

You: That was when Sarah was traveling for work

Hone: That makes sense. Want me to tag those as "solo weeks" 
       so you can track that pattern?
```

### Goal-Oriented Flow

```
You: Help me find $200/month to cut
Hone: Let's find $200. Here's what I see:
       QUICK WINS (low effort, immediate savings)
       ┌────────────────────────────────────────────┐
       │ ☐ Hulu — no activity in 67 days    $18/mo │
       │ ☐ iCloud 2TB — using 43GB          $10/mo │
       └────────────────────────────────────────────┘
       
       CONSIDER (might miss it)
       ┌────────────────────────────────────────────┐
       │ ☐ Spotify + Apple Music (have both) $26/mo │
       │ ☐ DoorDash → cooking more          ~$80/mo │
       └────────────────────────────────────────────┘
       
       Selected: $0/mo    Target: $200/mo
       
       Toggle items to see your savings add up.
You: [toggles Hulu, iCloud, Spotify]
Hone: Nice! That's $54/month ($648/year).
       
       Want me to set reminders to cancel these, or mark them 
       for cancellation tracking?
```
## Architecture
```
┌─────────────────────────────────────────────────────────────────┐
│                         User Query                               │
│                "Why did I spend so much this month?"            │
└─────────────────────────────────┬───────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Intent Parser                               │
│                                                                  │
│  1. Classify query type (question, command, goal)               │
│  2. Extract entities (time period, category, merchant, amount)  │
│  3. Detect follow-up vs new conversation                        │
│  4. Route to appropriate handler                                 │
└─────────────────────────────────┬───────────────────────────────┘
                                  │
            ┌─────────────────────┼─────────────────────┐
            ▼                     ▼                     ▼
     ┌──────────────┐    ┌───────────────┐    ┌───────────────┐
     │ Quick Query  │    │  Analysis     │    │  Goal Flow    │
     │   Handler    │    │   Handler     │    │   Handler     │
     │              │    │               │    │               │
     │ DB lookup    │    │ Run insight   │    │ Interactive   │
     │ Simple math  │    │ LLM narrative │    │ Multi-step    │
     │ No LLM       │    │ via Router    │    │ State mgmt    │
     └──────┬───────┘    └───────┬───────┘    └───────┬───────┘
            │                    │                    │
            └────────────────────┼────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Response Formatter                           │
│                                                                  │
│  • Structure data for display                                   │
│  • Generate narrative (LLM if needed)                           │
│  • Suggest follow-ups                                           │
│  • Track conversation context                                    │
└─────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                        UI Rendering                              │
│                                                                  │
│  Standard components: charts, tables, cards, toggles            │
│  (Generative UI only for complex custom visualizations)         │
└─────────────────────────────────────────────────────────────────┘
```
## Query Types
### 1. Quick Queries (No LLM needed)
Direct database lookups, simple aggregations.
| Pattern | Example | Handler |
|---------|---------|---------|
| Spending total | "How much did I spend at Target?" | Sum transactions |
| Count | "How many times did I eat out?" | Count by category |
| List | "What subscriptions do I have?" | List subscriptions |
| Lookup | "When is Netflix due?" | Subscription lookup |
| Comparison | "Groceries this month vs last?" | Period comparison |
These should respond in <100ms. No LLM required.
### 2. Analysis Queries (LLM-assisted)
Require interpretation, explanation, or pattern detection.
| Pattern | Example | Handler |
|---------|---------|---------|
| Explanation | "Why did spending increase?" | SpendingExplainer insight |
| Anomaly | "Anything unusual this month?" | Anomaly detection |
| Comparison | "How does this compare to normal?" | Baseline comparison |
| Recommendation | "Where should I cut back?" | SavingsOpportunity insight |
| Forecast | "What will next month look like?" | ExpenseForecaster insight |
These leverage the Insight Engine, with LLM providing narrative via Model Router.
### 3. Goal Flows (Multi-step)
Interactive sessions with state.
| Flow | Trigger | Steps |
|------|---------|-------|
| Savings finder | "Help me save $X" | Show options → toggle → confirm |
| Budget setup | "Help me set a budget" | Analyze → suggest limits → adjust |
| Category cleanup | "Help me organize tags" | Show untagged → suggest → confirm |
| Subscription audit | "Review my subscriptions" | List all → assess each → action |

These maintain state across turns and guide the user through a process.

## Intent Parser

### Classification

```rust
#[derive(Debug, Clone)]
pub enum QueryIntent {
    // Quick queries (no LLM)
    SpendingTotal { 
        merchant: Option<String>,
        category: Option<String>,
        period: TimePeriod,
    },
    TransactionCount {
        filter: TransactionFilter,
    },
    ListItems {
        item_type: ListType, // subscriptions, merchants, categories
    },
    
    // Analysis queries (LLM-assisted)
    Explain {
        subject: ExplainSubject, // spending change, transaction, pattern
        context: Option<String>,
    },
    Compare {
        a: TimePeriod,
        b: TimePeriod,
        scope: Option<String>, // category, merchant, all
    },
    Recommend {
        goal: RecommendGoal, // cut spending, optimize, find waste
    },
    Forecast {
        period: TimePeriod,
    },
    
    // Goal flows
    StartFlow {
        flow_type: FlowType,
        parameters: HashMap<String, String>,
    },
    ContinueFlow {
        action: String,
        selection: Option<Vec<String>>,
    },
    
    // Meta
    FollowUp {
        // Interpreted in context of previous exchange
        raw_query: String,
    },
    Unclear {
        raw_query: String,
        clarification_needed: String,
    },
}
```
### Entity Extraction
```rust
pub struct ExtractedEntities {
    pub time_period: Option<TimePeriod>,
    pub amount: Option<f64>,
    pub merchant: Option<String>,
    pub category: Option<String>,
    pub comparison: Option<ComparisonType>,
}

pub enum TimePeriod {
    ThisMonth,
    LastMonth,
    ThisYear,
    LastYear,
    Last { count: u32, unit: TimeUnit },
    DateRange { from: NaiveDate, to: NaiveDate },
    Relative { description: String }, // "when Sarah was traveling"
}
```
### Parser Implementation
Two-tier approach for speed:
```rust
impl IntentParser {
    pub fn parse(&self, query: &str, context: &ConversationContext) -> ParsedIntent {
        // Tier 1: Pattern matching (instant)
        if let Some(intent) = self.pattern_match(query) {
            return intent;
        }
        
        // Tier 2: LLM classification (fast model)
        self.llm_classify(query, context).await
    }
    
    fn pattern_match(&self, query: &str) -> Option<ParsedIntent> {
        let q = query.to_lowercase();
        
        // Spending queries
        if q.contains("how much") && q.contains("spend") {
            return Some(self.parse_spending_query(&q));
        }
        
        // List queries
        if q.starts_with("what") && q.contains("subscription") {
            return Some(ParsedIntent::ListItems { 
                item_type: ListType::Subscriptions 
            });
        }
        
        // Comparison queries
        if q.contains("vs") || q.contains("compared to") || q.contains("versus") {
            return Some(self.parse_comparison(&q));
        }
        
        // ... more patterns
        
        None
    }
    
    async fn llm_classify(&self, query: &str, context: &ConversationContext) -> ParsedIntent {
        let prompt = format!(
            "Classify this financial query and extract entities.\n\
             Query: \"{}\"\n\
             Previous context: {:?}\n\
             \n\
             Return JSON with: intent, entities, is_followup",
            query, context.summary()
        );
        
        self.router
            .with_task(TaskType::FastClassification)
            .system(INTENT_CLASSIFICATION_PROMPT)
            .prompt(&prompt)
            .call_json()
            .await
            .unwrap_or(ParsedIntent::Unclear { 
                raw_query: query.to_string(),
                clarification_needed: "I'm not sure what you're asking".to_string(),
            })
    }
}
```

## Conversation Context

Maintain context for follow-ups:

```rust
pub struct ConversationContext {
    pub session_id: String,
    pub exchanges: Vec<Exchange>,
    pub current_flow: Option<ActiveFlow>,
    pub referenced_entities: ReferencedEntities,
}

pub struct Exchange {
    pub query: String,
    pub intent: QueryIntent,
    pub response_summary: String,
    pub data_shown: DataSummary, // What we showed them
    pub timestamp: DateTime<Utc>,
}

pub struct ReferencedEntities {
    // Things we've talked about that can be referenced by "it", "that", etc.
    pub transactions: Vec<i64>,
    pub subscriptions: Vec<i64>,
    pub merchants: Vec<String>,
    pub categories: Vec<String>,
    pub time_period: Option<TimePeriod>,
    pub amount: Option<f64>,
}

impl ConversationContext {
    /// Resolve pronouns and references using context
    pub fn resolve_references(&self, query: &str) -> String {
        let mut resolved = query.to_string();
        
        // "it" / "that" → most recently discussed entity
        if query.contains(" it") || query.contains(" that") {
            if let Some(merchant) = self.referenced_entities.merchants.last() {
                resolved = resolved.replace(" it", &format!(" {}", merchant));
                resolved = resolved.replace(" that", &format!(" {}", merchant));
            }
        }
        
        // "there" → most recent merchant
        // "then" → most recent time period
        // etc.
        
        resolved
    }
    
    /// Infer missing context from conversation
    pub fn fill_defaults(&self, intent: &mut QueryIntent) {
        // If they ask "how much?" without specifying period, use last discussed
        if let QueryIntent::SpendingTotal { period, .. } = intent {
            if *period == TimePeriod::Unspecified {
                if let Some(prev_period) = &self.referenced_entities.time_period {
                    *period = prev_period.clone();
                }
            }
        }
    }
}
```

### Context Persistence

```sql
CREATE TABLE explore_sessions (
    id TEXT PRIMARY KEY,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_activity DATETIME DEFAULT CURRENT_TIMESTAMP,
    context_json TEXT  -- Serialized ConversationContext
);

CREATE TABLE explore_exchanges (
    id INTEGER PRIMARY KEY,
    session_id TEXT REFERENCES explore_sessions(id),
    query TEXT NOT NULL,
    intent_json TEXT,
    response_summary TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
-- Auto-cleanup old sessions
-- Sessions expire after 24 hours of inactivity
```
## Goal Flows
Interactive multi-step flows with state management.
### Flow Definition
```rust
pub trait GoalFlow: Send + Sync {
    fn id(&self) -> &'static str;
    fn trigger_phrases(&self) -> &[&'static str];
    
    /// Initialize the flow with parameters
    fn start(&self, params: &FlowParams, ctx: &AnalysisContext) -> FlowState;
    
    /// Process user action and advance flow
    fn step(&self, state: &FlowState, action: &FlowAction) -> FlowState;
    
    /// Render current state for display
    fn render(&self, state: &FlowState) -> FlowResponse;
    
    /// Check if flow is complete
    fn is_complete(&self, state: &FlowState) -> bool;
}
pub struct FlowState {
    pub step: String,
    pub data: serde_json::Value,
    pub selections: Vec<String>,
    pub computed: serde_json::Value, // Running totals, etc.
}

pub struct FlowResponse {
    pub message: String,
    pub options: Vec<FlowOption>,
    pub visualization: Option<Visualization>,
    pub actions: Vec<FlowAction>,
}
```

### Savings Finder Flow

```rust
pub struct SavingsFinderFlow;
impl GoalFlow for SavingsFinderFlow {
    fn id(&self) -> &'static str { "savings_finder" }
    
    fn trigger_phrases(&self) -> &[&'static str] {
        &[
            "help me save",
            "find savings",
            "cut spending",
            "reduce expenses",
            "where can i cut",
        ]
    }
    
    fn start(&self, params: &FlowParams, ctx: &AnalysisContext) -> FlowState {
        let target = params.get("amount")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(100.0);
        
        // Find savings opportunities
        let opportunities = self.find_opportunities(ctx);
        
        FlowState {
            step: "select".to_string(),
            data: json!({
                "target": target,
                "opportunities": opportunities,
            }),
            selections: vec![],
            computed: json!({ "selected_total": 0.0 }),
        }
    }
    
    fn step(&self, state: &FlowState, action: &FlowAction) -> FlowState {
        match action {
            FlowAction::Toggle { item_id } => {
                let mut new_state = state.clone();
                
                if new_state.selections.contains(item_id) {
                    new_state.selections.retain(|s| s != item_id);
                } else {
                    new_state.selections.push(item_id.clone());
                }
                
                // Recalculate total
                let total = self.calculate_selected_total(&new_state);
                new_state.computed = json!({ "selected_total": total });
                
                new_state
            }
            FlowAction::Confirm => {
                let mut new_state = state.clone();
                new_state.step = "confirmed".to_string();
                new_state
            }
            _ => state.clone(),
        }
    }
    
    fn render(&self, state: &FlowState) -> FlowResponse {
        let target: f64 = state.data["target"].as_f64().unwrap_or(100.0);
        let selected: f64 = state.computed["selected_total"].as_f64().unwrap_or(0.0);
        let opportunities: Vec<Opportunity> = serde_json::from_value(
            state.data["opportunities"].clone()
        ).unwrap_or_default();
        
        FlowResponse {
            message: format!(
                "Selected: ${:.0}/month    Target: ${:.0}/month",
                selected, target
            ),
            options: opportunities.iter().map(|o| FlowOption {
                id: o.id.clone(),
                label: o.description.clone(),
                amount: o.monthly_amount,
                selected: state.selections.contains(&o.id),
                category: o.category.clone(),
            }).collect(),
            visualization: Some(Visualization::Progress {
                current: selected,
                target,
            }),
            actions: vec![
                FlowAction::Confirm,
                FlowAction::Cancel,
            ],
        }
    }
}
```
## Response Generation
### Narrative Layer
LLM generates natural language around structured data:
```rust
impl ResponseGenerator {
    pub async fn generate(
        &self,
        intent: &QueryIntent,
        data: &QueryResult,
        context: &ConversationContext,
        router: &ModelRouter,
    ) -> Response {
        // Structure the data first
        let structured = self.structure_response(intent, data);
        
        // Generate narrative if needed
        let narrative = if self.needs_narrative(intent) {
            router
                .with_task(TaskType::Narrative)
                .system(RESPONSE_NARRATIVE_PROMPT)
                .prompt(&format!(
                    "Generate a helpful response for this query.\n\
                     Query: {:?}\n\
                     Data: {:?}\n\
                     Conversation context: {:?}",
                    intent, data, context.summary()
                ))
                .call()
                .await
                .ok()
        } else {
            None
        };
        
        // Suggest follow-ups
        let suggestions = self.suggest_followups(intent, data, context);
        
        Response {
            narrative,
            data: structured,
            suggestions,
            visualization: self.pick_visualization(intent, data),
        }
    }
    
    fn suggest_followups(&self, intent: &QueryIntent, data: &QueryResult, ctx: &ConversationContext) -> Vec<String> {
        match intent {
            QueryIntent::SpendingTotal { category: Some(cat), .. } => vec![
                format!("Break down {} by merchant", cat),
                format!("Compare {} to last month", cat),
                format!("Show {} transactions", cat),
            ],
            QueryIntent::Explain { subject: ExplainSubject::SpendingChange, .. } => vec![
                "What can I do about it?".to_string(),
                "Is this a trend?".to_string(),
                "Show me the transactions".to_string(),
            ],
            _ => vec![],
        }
    }
}
```
## API Design
### REST Endpoints
```
POST /api/explore/query
{
    "query": "Where did my money go last month?",
    "session_id": "optional-session-id"
}

Response:
{
    "session_id": "abc123",
    "response": {
        "narrative": "Last month you spent $4,230...",
        "data": { ... },
        "visualization": { "type": "bar_chart", ... },
        "suggestions": ["Break down by merchant", "Compare to last month"]
    },
    "intent": "spending_total",
    "processing_time_ms": 245
}
```

### Flow Interaction

```
POST /api/explore/flow/start
{
    "flow": "savings_finder",
    "params": { "amount": "200" }
}
POST /api/explore/flow/action
{
    "session_id": "abc123",
    "action": { "type": "toggle", "item_id": "sub_hulu" }
}
POST /api/explore/flow/confirm
{
    "session_id": "abc123"
}
```
### Streaming (Future)
For longer responses, stream tokens:
```
POST /api/explore/query?stream=true

Response: Server-Sent Events
data: {"type": "narrative_chunk", "text": "Last month "}
data: {"type": "narrative_chunk", "text": "you spent "}
data: {"type": "data", "data": {...}}
data: {"type": "done"}
```
## UI Components
### Chat Interface
```tsx
function ExploreChat() {
    const [messages, setMessages] = useState<Message[]>([]);
    const [input, setInput] = useState('');
    const [sessionId, setSessionId] = useState<string | null>(null);
    
    const sendQuery = async () => {
        const userMessage = { role: 'user', content: input };
        setMessages(prev => [...prev, userMessage]);
        setInput('');
        
        const response = await fetch('/api/explore/query', {
            method: 'POST',
            body: JSON.stringify({ 
                query: input, 
                session_id: sessionId 
            }),
        });
        
        const data = await response.json();
        setSessionId(data.session_id);
        
        const assistantMessage = {
            role: 'assistant',
            content: data.response.narrative,
            data: data.response.data,
            visualization: data.response.visualization,
            suggestions: data.response.suggestions,
        };
        
        setMessages(prev => [...prev, assistantMessage]);
    };
    
    return (
        <div className="flex flex-col h-full">
            <div className="flex-1 overflow-y-auto p-4 space-y-4">
                {messages.map((msg, i) => (
                    <MessageBubble key={i} message={msg} />
                ))}
            </div>
            
            <div className="border-t p-4">
                <SuggestionChips 
                    suggestions={messages.at(-1)?.suggestions || []}
                    onSelect={setInput}
                />
                <ChatInput 
                    value={input}
                    onChange={setInput}
                    onSend={sendQuery}
                />
            </div>
        </div>
    );
}
```

### Visualization Components

Standard components that render based on `visualization` type:

```tsx
function ResponseVisualization({ viz }: { viz: Visualization }) {
    switch (viz.type) {
        case 'bar_chart':
            return <BarChart data={viz.data} />;
        case 'breakdown':
            return <SpendingBreakdown segments={viz.segments} />;
        case 'comparison':
            return <ComparisonTable items={viz.items} />;
        case 'progress':
            return <ProgressBar current={viz.current} target={viz.target} />;
        case 'timeline':
            return <TransactionTimeline events={viz.events} />;
        default:
            return null;
    }
}
```
## When to Use Generative UI
Most queries use standard components. Reserve full UI generation for:
1. **Truly novel visualizations** — User asks for something we don't have a component for
2. **Complex custom dashboards** — "Show me a dashboard for my side business expenses"
3. **Export/presentation** — "Create a spending report I can share"
Detection:
```rust
fn should_generate_ui(intent: &QueryIntent, complexity: f32) -> bool {
    matches!(intent, 
        QueryIntent::CustomVisualization { .. } |
        QueryIntent::CreateReport { .. }
    ) || complexity > 0.8
}
```

When triggered, falls back to the Generative UI approach from the original design.

## Integration Points

### With Insight Engine

Explore Mode can surface insights on demand:

```rust
// "Anything unusual this month?"
QueryIntent::Anomaly { period } => {
    let insights = insight_engine
        .analyze_for_period(period)
        .filter(|i| i.severity >= Severity::Attention);
    
    // Render as conversational response
}
```
### With Model Router
All LLM calls go through the router:
```rust
// Intent classification → FastClassification
// Spending explanation → Reasoning  
// Response narrative → Narrative
// Entity extraction → StructuredExtraction
```

## Implementation Phases

### Phase 1: Quick Queries
- [ ] Pattern-based intent parsing (no LLM)
- [ ] Spending total, count, list handlers
- [ ] Basic REST API
- [ ] Simple chat UI

### Phase 2: Context & Follow-ups
- [ ] Conversation context tracking
- [ ] Reference resolution ("it", "that", "there")
- [ ] Session persistence
- [ ] Follow-up suggestions

### Phase 3: Analysis Queries
- [ ] LLM intent classification (fast model)
- [ ] Integration with Insight Engine
- [ ] Narrative generation
- [ ] Visualization selection

### Phase 4: Goal Flows
- [ ] Flow framework
- [ ] Savings finder flow
- [ ] Subscription audit flow
- [ ] Flow UI components

### Phase 5: Polish
- [ ] Streaming responses
- [ ] Voice input (future)
- [ ] Generative UI fallback for complex requests
- [ ] Mobile-optimized chat UI

## Example Queries

| Query | Type | LLM? | Response Time |
|-------|------|------|---------------|
| "How much at Target?" | Quick | No | <100ms |
| "Subscriptions?" | Quick | No | <100ms |
| "Compare this month to last" | Quick | No | <200ms |
| "Why did I spend more?" | Analysis | Yes | 1-3s |
| "What's unusual?" | Analysis | Yes | 1-3s |
| "Help me save $200" | Flow | Partial | Interactive |
| "Why?" (follow-up) | Context | Maybe | Depends |
| "Create a report for taxes" | Generative | Yes | 10-30s |
## Implementation Status
### MCP Server (Phase 0) ✅
The MCP server is implemented and ready for use. See [mcp.md](../mcp.md) for setup instructions.
- [x] Add `--mcp-port` flag to `hone serve`
- [x] MCP server with HTTP/SSE transport (rmcp + Streamable HTTP)
- [x] `search_transactions` tool
- [x] `get_spending_summary` tool
- [x] `get_subscriptions` tool
- [x] `get_alerts` tool
- [x] `compare_spending` tool
- [x] `get_merchants` tool
- [x] `get_account_summary` tool
- [x] Documentation with Claude Desktop config

**Usage:**
```bash
hone serve --port 3000 --mcp-port 3001 --host 0.0.0.0
```
### In-App Explore Mode (Phase 1) ✅
LLM-powered conversational interface implemented:
- [x] `POST /api/explore/query` endpoint
- [x] Prompt library integration (`explore_agent.md`)
- [x] AIOrchestrator with all MCP tools (search_transactions, get_spending_summary, etc.)
- [x] Chat UI with message history
- [x] Suggestion chips for common queries
- [x] Processing time display

**Usage:**
```bash
export ANTHROPIC_COMPATIBLE_HOST="http://localhost:11434"
export ANTHROPIC_COMPATIBLE_MODEL="qwen3-coder"
hone serve --port 3000
```
Navigate to `#/explore` in the web UI.
### Session Persistence (Phase 2) ✅
Multi-turn conversation support with server-side session management:
- [x] `AIOrchestrator.execute_with_history()` for conversation context
- [x] `ExploreSessionManager` for in-memory session storage
- [x] Session auto-creation on first query
- [x] Session ID returned in response for follow-up queries
- [x] 30-minute session timeout with auto-cleanup
- [x] Message history capped at 20 messages to limit context size
- [x] "New conversation" button to clear session
- [x] `POST /api/explore/session` - Create new session
- [x] `GET /api/explore/session/:id` - Get session info
- [x] `DELETE /api/explore/session/:id` - Delete session

**How it works:**
1. First query creates a session automatically
2. Response includes `session_id`
3. Frontend passes `session_id` in subsequent queries
4. Server reconstructs conversation history from prior messages
5. LLM sees full context for follow-ups like "tell me more" or "why?"
### Model Selector ✅
Users can switch between available Ollama models within Explore Mode:
- [x] `GET /api/explore/models` - List available models from Ollama
- [x] Model dropdown in UI header
- [x] Selected model passed with each query
- [x] Default model highlighted in dropdown
- [x] Model name shown in response metadata
**API Endpoints:**
```
GET /api/explore/models
{
  "models": ["qwen3-coder:latest", "llama3.2:latest", ...],
  "default_model": "qwen3-coder:latest"
}
POST /api/explore/query
{
  "query": "What did I spend last month?",
  "session_id": "optional",
  "model": "optional-override-model"
}
```
### Future Enhancements
Phases 3-5 as described above:
- Reference resolution ("it", "that")
- Goal flows (savings finder, etc.)
- Streaming responses