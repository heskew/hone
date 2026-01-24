import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import { ChevronDown, Settings, ExternalLink, ChevronRight, Brain, Database, Sparkles } from "lucide-react";
import { api } from "../../api";
import type { ExploreResponse } from "../../types";

// Workflow states for agentic processing
type WorkflowState = "thinking" | "querying" | "analyzing";

const WORKFLOW_CONFIG: Record<WorkflowState, { label: string; icon: "brain" | "database" | "sparkles"; duration: number }> = {
  thinking: { label: "Thinking", icon: "brain", duration: 2500 },
  querying: { label: "Querying data", icon: "database", duration: 2000 },
  analyzing: { label: "Analyzing", icon: "sparkles", duration: 2500 },
};

// Order of states in the cycle
const WORKFLOW_CYCLE: WorkflowState[] = ["thinking", "querying", "analyzing", "thinking"];

// Component that animates through workflow states
function ThinkingIndicator() {
  const [stateIndex, setStateIndex] = useState(0);
  const [elapsed, setElapsed] = useState(0);
  const [isTransitioning, setIsTransitioning] = useState(false);

  const currentState = WORKFLOW_CYCLE[stateIndex % WORKFLOW_CYCLE.length];
  const config = WORKFLOW_CONFIG[currentState];

  useEffect(() => {
    // Cycle through states with configured durations
    const timer = setTimeout(() => {
      setIsTransitioning(true);
      setTimeout(() => {
        setStateIndex((prev) => prev + 1);
        setIsTransitioning(false);
      }, 150); // Brief fade transition
    }, config.duration);

    return () => clearTimeout(timer);
  }, [stateIndex, config.duration]);

  useEffect(() => {
    // Update elapsed time every second
    const elapsedInterval = setInterval(() => {
      setElapsed((prev) => prev + 1);
    }, 1000);

    return () => clearInterval(elapsedInterval);
  }, []);

  const Icon = config.icon === "brain" ? Brain : config.icon === "database" ? Database : Sparkles;

  return (
    <div className="flex items-center gap-3">
      {/* Animated icon */}
      <div
        className={`transition-all duration-150 ${isTransitioning ? "opacity-0 scale-75" : "opacity-100 scale-100"}`}
      >
        <Icon className="w-4 h-4 text-hone-500 dark:text-hone-400 animate-pulse" />
      </div>

      {/* Status text */}
      <div className="flex items-center gap-2">
        <span
          className={`text-hone-600 dark:text-hone-300 transition-opacity duration-150 ${
            isTransitioning ? "opacity-0" : "opacity-100"
          }`}
        >
          {config.label}
        </span>

        {/* Animated dots */}
        <span className="flex gap-0.5">
          {[0, 1, 2].map((i) => (
            <span
              key={i}
              className="w-1 h-1 bg-hone-400 dark:bg-hone-500 rounded-full animate-bounce"
              style={{ animationDelay: `${i * 150}ms`, animationDuration: "1s" }}
            />
          ))}
        </span>
      </div>

      {/* Elapsed time (show after 5s) */}
      {elapsed >= 5 && (
        <span className="text-xs text-hone-400 tabular-nums">({elapsed}s)</span>
      )}
    </div>
  );
}

interface Message {
  role: "user" | "assistant";
  content: string;
  processingTime?: number;
  model?: string;
}

// Parsed content from AI response
interface ParsedContent {
  // The human-readable text (without JSON)
  text: string;
  // Extracted data that can be linked
  data: ExtractedData | null;
}

interface ExtractedData {
  type: "spending" | "transactions" | "subscriptions" | "merchants" | "unknown";
  // For building links
  tag?: string;
  period?: string;
  totalAmount?: number;
  transactionCount?: number;
  // Raw data for display
  raw?: unknown;
}

// Parse response to separate JSON from text and extract linkable data
function parseResponse(content: string): ParsedContent {
  let cleanedText = content;
  let extractedData: ExtractedData | null = null;

  // Pattern 1: JSON array at start (tool result leak from some models)
  // e.g., [{"response":"{\"total_spending\":...}"}]
  const jsonArrayMatch = content.match(/^\s*\[(\{[^]*?\})\]\s*/);
  if (jsonArrayMatch) {
    try {
      const jsonStr = `[${jsonArrayMatch[1]}]`;
      const parsed = JSON.parse(jsonStr);
      cleanedText = content.slice(jsonArrayMatch[0].length).trim();

      // Try to identify the data type
      if (parsed[0]?.response) {
        // This is a wrapped response object
        const inner = JSON.parse(parsed[0].response);
        extractedData = extractDataInfo(inner);
      } else {
        extractedData = extractDataInfo(parsed);
      }
    } catch {
      // Failed to parse, continue
    }
  }

  // Pattern 2: Inline tool call JSON (llama3.3 style)
  // e.g., {"name": "search_transactions", "parameters": {...}}
  const toolCallMatch = cleanedText.match(/\{"name":\s*"[^"]+",\s*"parameters":\s*\{[^}]*\}\}/g);
  if (toolCallMatch) {
    // Remove all tool call JSON from the text
    for (const match of toolCallMatch) {
      cleanedText = cleanedText.replace(match, "").trim();
    }
    // Clean up any double spaces or line breaks left behind
    cleanedText = cleanedText.replace(/\n\s*\n/g, "\n").trim();
  }

  // Pattern 3: JSON object at start (spending summary, etc.)
  if (!extractedData) {
    const jsonObjMatch = cleanedText.match(/^\s*\{[^]*?\}\s*/);
    if (jsonObjMatch) {
      try {
        const parsed = JSON.parse(jsonObjMatch[0]);
        // Only extract if it looks like data, not a tool call
        if (!("name" in parsed && "parameters" in parsed)) {
          cleanedText = cleanedText.slice(jsonObjMatch[0].length).trim();
          extractedData = extractDataInfo(parsed);
        }
      } catch {
        // Failed to parse
      }
    }
  }

  // Pattern 4: Embedded JSON in text (tool results that leaked mid-response)
  // Look for JSON objects that look like spending summaries, transaction lists, etc.
  const embeddedJsonMatch = cleanedText.match(/\{"total_spending":[^}]+,"categories":\[[^\]]*\]\}/);
  if (embeddedJsonMatch && !extractedData) {
    try {
      const parsed = JSON.parse(embeddedJsonMatch[0]);
      cleanedText = cleanedText.replace(embeddedJsonMatch[0], "").trim();
      extractedData = extractDataInfo(parsed);
    } catch {
      // Failed to parse
    }
  }

  return { text: cleanedText, data: extractedData };
}

function extractDataInfo(data: unknown): ExtractedData | null {
  if (!data || typeof data !== "object") return null;

  const obj = data as Record<string, unknown>;

  // Spending summary response
  if ("total_spending" in obj && "categories" in obj) {
    const categories = obj.categories as Array<{ category: string; amount: number; transaction_count?: number }>;
    const mainCategory = categories?.[0]?.category;
    return {
      type: "spending",
      tag: mainCategory,
      totalAmount: obj.total_spending as number,
      transactionCount: categories?.[0]?.transaction_count,
    };
  }

  // Transaction list response
  if (Array.isArray(data) && data.length > 0 && "id" in (data[0] as object)) {
    return {
      type: "transactions",
      transactionCount: data.length,
    };
  }

  // Subscriptions response
  if (Array.isArray(data) && data.length > 0 && "merchant" in (data[0] as object) && "amount" in (data[0] as object)) {
    return {
      type: "subscriptions",
    };
  }

  // Merchants response
  if (Array.isArray(data) && data.length > 0 && "merchant" in (data[0] as object) && "total" in (data[0] as object)) {
    return {
      type: "merchants",
    };
  }

  return { type: "unknown", raw: data };
}

// Component to render extracted data with links
function DataCard({ data }: { data: ExtractedData }) {
  const buildLink = () => {
    if (data.type === "spending" && data.tag) {
      // Link to transactions filtered by tag
      return `#/transactions?tags=${encodeURIComponent(data.tag)}`;
    }
    if (data.type === "subscriptions") {
      return "#/subscriptions";
    }
    if (data.type === "merchants") {
      return "#/reports?tab=merchants";
    }
    if (data.type === "transactions") {
      return "#/transactions";
    }
    return null;
  };

  const link = buildLink();
  const label = {
    spending: "View transactions",
    transactions: `View ${data.transactionCount || ""} transactions`,
    subscriptions: "View subscriptions",
    merchants: "View merchants report",
    unknown: null,
  }[data.type];

  if (!link || !label) return null;

  return (
    <a
      href={link}
      target="_blank"
      rel="noopener noreferrer"
      className="inline-flex items-center gap-1 mt-2 px-2 py-1 text-xs
                 bg-hone-200 dark:bg-hone-700 text-hone-700 dark:text-hone-300
                 rounded hover:bg-hone-300 dark:hover:bg-hone-600 transition-colors"
    >
      {label}
      <ExternalLink className="w-3 h-3" />
    </a>
  );
}

// Component to render a collapsible raw data section
function RawDataSection({ data }: { data: unknown }) {
  const [expanded, setExpanded] = useState(false);

  if (!data) return null;

  return (
    <div className="mt-2 text-xs">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1 text-hone-500 dark:text-hone-400 hover:text-hone-700 dark:hover:text-hone-300"
      >
        <ChevronRight className={`w-3 h-3 transition-transform ${expanded ? "rotate-90" : ""}`} />
        Raw data
      </button>
      {expanded && (
        <pre className="mt-1 p-2 bg-hone-50 dark:bg-hone-900 rounded text-xs overflow-x-auto max-h-40">
          {JSON.stringify(data, null, 2)}
        </pre>
      )}
    </div>
  );
}

// Render message content with parsed data
function MessageContent({ content }: { content: string }) {
  const parsed = useMemo(() => parseResponse(content), [content]);

  return (
    <div>
      <div className="whitespace-pre-wrap">{parsed.text}</div>
      {parsed.data && (
        <>
          <DataCard data={parsed.data} />
          {parsed.data.type === "unknown" && parsed.data.raw && (
            <RawDataSection data={parsed.data.raw} />
          )}
        </>
      )}
    </div>
  );
}

const SUGGESTIONS = [
  "What did I spend last month?",
  "Show my subscriptions",
  "Any zombie subscriptions?",
  "Compare this month to last month",
  "What are my top merchants?",
];

export function ExplorePage() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Model selection state
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [defaultModel, setDefaultModel] = useState<string>("");
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [showModelSelector, setShowModelSelector] = useState(false);
  const [loadingModels, setLoadingModels] = useState(false);

  // Load available models on mount
  useEffect(() => {
    const loadModels = async () => {
      setLoadingModels(true);
      try {
        const response = await api.getExploreModels();
        setAvailableModels(response.models);
        setDefaultModel(response.default_model);
        setSelectedModel(response.default_model);
      } catch (err) {
        console.error("Failed to load models:", err);
        // Don't show error to user - they can still use default model
      } finally {
        setLoadingModels(false);
      }
    };
    loadModels();
  }, []);

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const sendQuery = async (query: string) => {
    if (!query.trim()) return;

    setError(null);
    const userMessage: Message = { role: "user", content: query };
    setMessages((prev) => [...prev, userMessage]);
    setInput("");
    setLoading(true);

    try {
      // Pass session ID if we have one, and model if different from default
      const modelToUse = selectedModel !== defaultModel ? selectedModel : undefined;
      const response: ExploreResponse = await api.queryExplore(
        query,
        sessionId ?? undefined,
        modelToUse
      );

      // Store the session ID for future queries
      if (response.session_id) {
        setSessionId(response.session_id);
      }

      const assistantMessage: Message = {
        role: "assistant",
        content: response.response,
        processingTime: response.processing_time_ms,
        model: response.model,
      };
      setMessages((prev) => [...prev, assistantMessage]);
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to get response";
      setError(errorMessage);
    } finally {
      setLoading(false);
      // Re-focus input for easy follow-up
      inputRef.current?.focus();
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    sendQuery(input);
  };

  const handleNewConversation = useCallback(async () => {
    // Clear the current session on the server if we have one
    if (sessionId) {
      try {
        await api.clearExploreSession(sessionId);
      } catch {
        // Ignore errors - session might already be expired
      }
    }
    // Reset local state
    setMessages([]);
    setSessionId(null);
    setError(null);
    setInput("");
  }, [sessionId]);

  return (
    <div className="flex flex-col h-[calc(100vh-12rem)]">
      {/* Header with model selector and new conversation button */}
      <div className="flex items-center justify-between p-2 border-b border-hone-200 dark:border-hone-700">
        {/* Model selector */}
        <div className="relative">
          <button
            onClick={() => setShowModelSelector(!showModelSelector)}
            disabled={loadingModels || availableModels.length === 0}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm
                       text-hone-600 dark:text-hone-400
                       hover:text-hone-800 dark:hover:text-hone-200
                       hover:bg-hone-100 dark:hover:bg-hone-800
                       rounded transition-colors disabled:opacity-50"
            title="Select model"
          >
            <Settings className="w-4 h-4" />
            <span className="max-w-[150px] truncate">
              {loadingModels ? "Loading..." : selectedModel || "Select model"}
            </span>
            <ChevronDown className="w-3 h-3" />
          </button>

          {showModelSelector && availableModels.length > 0 && (
            <div className="absolute left-0 top-full mt-1 z-20
                            bg-white dark:bg-hone-800 border border-hone-200 dark:border-hone-700
                            rounded-lg shadow-lg py-1 min-w-[200px] max-h-[300px] overflow-y-auto">
              {availableModels.map((model) => (
                <button
                  key={model}
                  onClick={() => {
                    setSelectedModel(model);
                    setShowModelSelector(false);
                  }}
                  className={`w-full text-left px-3 py-2 text-sm
                              hover:bg-hone-100 dark:hover:bg-hone-700
                              ${model === selectedModel
                                ? "bg-hone-50 dark:bg-hone-700 text-hone-900 dark:text-hone-100"
                                : "text-hone-700 dark:text-hone-300"
                              }`}
                >
                  <span className="truncate block">{model}</span>
                  {model === defaultModel && (
                    <span className="text-xs text-hone-400 dark:text-hone-500">
                      (default)
                    </span>
                  )}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* New conversation button */}
        {messages.length > 0 && (
          <button
            onClick={handleNewConversation}
            className="px-3 py-1.5 text-sm text-hone-600 dark:text-hone-400
                       hover:text-hone-800 dark:hover:text-hone-200
                       hover:bg-hone-100 dark:hover:bg-hone-800
                       rounded transition-colors"
          >
            New conversation
          </button>
        )}
      </div>

      {/* Messages area */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.length === 0 && (
          <div className="text-center py-12">
            <h2 className="text-xl font-semibold text-hone-900 dark:text-hone-100 mb-2">
              Explore your finances
            </h2>
            <p className="text-hone-600 dark:text-hone-400 mb-6">
              Ask questions about your spending, subscriptions, and more.
            </p>
            <div className="flex flex-wrap justify-center gap-2">
              {SUGGESTIONS.map((suggestion) => (
                <button
                  key={suggestion}
                  onClick={() => sendQuery(suggestion)}
                  className="px-3 py-1.5 text-sm bg-hone-100 dark:bg-hone-800
                             text-hone-700 dark:text-hone-300 rounded-full
                             hover:bg-hone-200 dark:hover:bg-hone-700 transition-colors"
                >
                  {suggestion}
                </button>
              ))}
            </div>
          </div>
        )}

        {messages.map((msg, i) => (
          <div
            key={i}
            className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}
          >
            <div
              className={`max-w-[80%] rounded-lg px-4 py-2 ${
                msg.role === "user"
                  ? "bg-hone-600 text-white"
                  : "bg-hone-100 dark:bg-hone-800 text-hone-900 dark:text-hone-100"
              }`}
            >
              {msg.role === "user" ? (
                <div className="whitespace-pre-wrap">{msg.content}</div>
              ) : (
                <MessageContent content={msg.content} />
              )}
              {msg.role === "assistant" && (msg.processingTime !== undefined || msg.model) && (
                <div className="text-xs opacity-60 mt-1 flex items-center gap-2">
                  {msg.processingTime !== undefined && (
                    <span>{(msg.processingTime / 1000).toFixed(1)}s</span>
                  )}
                  {msg.model && (
                    <span className="truncate max-w-[120px]" title={msg.model}>
                      {msg.model}
                    </span>
                  )}
                </div>
              )}
            </div>
          </div>
        ))}

        {loading && (
          <div className="flex justify-start">
            <div className="bg-hone-100 dark:bg-hone-800 rounded-lg px-4 py-2">
              <ThinkingIndicator />
            </div>
          </div>
        )}

        {error && (
          <div className="text-center text-red-600 dark:text-red-400 py-2">
            {error}
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Input area */}
      <div className="border-t border-hone-200 dark:border-hone-700 p-4">
        <form onSubmit={handleSubmit} className="flex gap-2">
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Ask about your finances..."
            className="flex-1 px-4 py-2 rounded-lg border border-hone-300 dark:border-hone-600
                       bg-white dark:bg-hone-900 text-hone-900 dark:text-hone-100
                       focus:outline-none focus:ring-2 focus:ring-hone-500"
            disabled={loading}
          />
          <button
            type="submit"
            disabled={loading || !input.trim()}
            className="px-4 py-2 bg-hone-600 text-white rounded-lg
                       hover:bg-hone-700 disabled:opacity-50 disabled:cursor-not-allowed
                       transition-colors"
          >
            Send
          </button>
        </form>
      </div>
    </div>
  );
}
