import {
  Activity,
  AlertTriangle,
  ArrowDownRight,
  ArrowUpRight,
  BarChart3,
  CheckCircle,
  ChevronDown,
  ChevronRight,
  Clock,
  Cpu,
  RefreshCw,
  Server,
  XCircle,
  Zap,
} from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api";
import type {
  ModelComparisonStats,
  ModelRecommendation,
  ModelStats,
  OllamaHealthStatus,
  OllamaMetric,
  OllamaStats,
  ToolCallRecord,
} from "../../types";

type Tab = "overview" | "compare" | "calls";

export function OllamaPage() {
  const [tab, setTab] = useState<Tab>("overview");
  const [stats, setStats] = useState<OllamaStats | null>(null);
  const [health, setHealth] = useState<OllamaHealthStatus | null>(null);
  const [recommendation, setRecommendation] = useState<ModelRecommendation | null>(null);
  const [calls, setCalls] = useState<OllamaMetric[]>([]);
  const [expandedCalls, setExpandedCalls] = useState<Set<number>>(new Set());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [period, setPeriod] = useState("last-30-days");
  const [callsFilter, setCallsFilter] = useState<"all" | "failures">("all");
  const [operationFilter, setOperationFilter] = useState<string>("all");
  const [callsLimit, setCallsLimit] = useState(100);
  const [loadingMore, setLoadingMore] = useState(false);
  const [hasMoreCalls, setHasMoreCalls] = useState(true);

  // Model comparison state
  const [comparisonStats, setComparisonStats] = useState<ModelComparisonStats | null>(null);
  const [comparisonLoading, setComparisonLoading] = useState(false);
  const [expandedModels, setExpandedModels] = useState<Set<string>>(new Set());

  const loadData = async () => {
    try {
      setLoading(true);
      setError(null);

      const initialLimit = 100;
      const [statsData, healthData, recData, callsData] = await Promise.all([
        api.getOllamaStats(period),
        api.getOllamaHealth(),
        api.getOllamaRecommendation(),
        api.getOllamaCalls(initialLimit),
      ]);

      setStats(statsData);
      setHealth(healthData);
      setRecommendation(recData);
      setCalls(callsData);
      setCallsLimit(initialLimit);
      setHasMoreCalls(callsData.length >= initialLimit);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load Ollama data");
    } finally {
      setLoading(false);
    }
  };

  const loadMoreCalls = async () => {
    try {
      setLoadingMore(true);
      const newLimit = callsLimit + 100;
      const callsData = await api.getOllamaCalls(newLimit);
      setCalls(callsData);
      setCallsLimit(newLimit);
      setHasMoreCalls(callsData.length >= newLimit);
    } catch (err) {
      console.error("Failed to load more calls:", err);
    } finally {
      setLoadingMore(false);
    }
  };

  const loadComparisonData = async () => {
    try {
      setComparisonLoading(true);
      const data = await api.getOllamaStatsByModel(period);
      setComparisonStats(data);
    } catch (err) {
      console.error("Failed to load comparison data:", err);
    } finally {
      setComparisonLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, [period]);

  // Load comparison data when switching to compare tab
  useEffect(() => {
    if (tab === "compare" && !comparisonStats) {
      loadComparisonData();
    }
  }, [tab]);

  // Reload comparison data when period changes and on compare tab
  useEffect(() => {
    if (tab === "compare") {
      loadComparisonData();
    }
  }, [period]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <RefreshCw className="w-5 h-5 animate-spin text-hone-400" />
        <span className="ml-2 text-hone-500">Loading AI metrics...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="card p-8 text-center">
        <AlertTriangle className="w-12 h-12 text-attention mx-auto mb-4" />
        <h2 className="text-lg font-semibold mb-2">Failed to Load Metrics</h2>
        <p className="text-hone-500 mb-4">{error}</p>
        <button onClick={loadData} className="btn-primary">
          <RefreshCw className="w-4 h-4 mr-2" />
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold flex items-center gap-2">
          <Cpu className="w-6 h-6" />
          AI Metrics
        </h1>
        <p className="text-hone-500 text-sm mt-1">
          Monitor AI model performance and quality
        </p>
      </div>

      {/* Health Status */}
      {health && (
        <div className="space-y-3">
          {/* Classification Mode */}
          <div className={`card p-4 flex items-center gap-4 ${health.available ? "border-savings/30" : "border-waste/30"}`}>
            {health.available ? (
              <CheckCircle className="w-8 h-8 text-savings" />
            ) : (
              <XCircle className="w-8 h-8 text-waste" />
            )}
            <div className="flex-1">
              <div className="font-medium">
                {health.available ? "Classification Mode" : "Classification Mode Unavailable"}
              </div>
              <div className="text-sm text-hone-500">
                {health.host || "No host configured"}
                {health.model && ` - Model: ${health.model}`}
              </div>
            </div>
            {health.recent_error_rate > 0 && (
              <div className="text-sm text-attention">
                {(health.recent_error_rate * 100).toFixed(1)}% recent errors
              </div>
            )}
          </div>

          {/* Agentic Mode */}
          <div className={`card p-4 flex items-center gap-4 ${health.orchestrator_available ? "border-savings/30" : "border-hone-200 dark:border-hone-700"}`}>
            {health.orchestrator_available ? (
              <Zap className="w-8 h-8 text-savings" />
            ) : (
              <Zap className="w-8 h-8 text-hone-300" />
            )}
            <div className="flex-1">
              <div className="font-medium">
                {health.orchestrator_available ? "Agentic Mode" : "Agentic Mode Not Configured"}
              </div>
              <div className="text-sm text-hone-500">
                {health.orchestrator_available ? (
                  <>
                    {health.orchestrator_host}
                    {health.orchestrator_model && ` - Model: ${health.orchestrator_model}`}
                  </>
                ) : (
                  "Set ANTHROPIC_COMPATIBLE_HOST for tool-calling analysis"
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Recommendation Card */}
      {recommendation && (
        <div className={`card p-4 ${recommendation.should_switch ? "border-attention/30" : ""}`}>
          <div className="flex items-start gap-3">
            <Zap className={`w-5 h-5 mt-0.5 ${recommendation.should_switch ? "text-attention" : "text-savings"}`} />
            <div className="flex-1">
              <div className="font-medium mb-2">
                {recommendation.should_switch ? "Action Recommended" : "Performance Good"}
              </div>
              <ul className="text-sm text-hone-600 dark:text-hone-400 space-y-1">
                {recommendation.recommendations.map((rec, i) => (
                  <li key={i}>{rec}</li>
                ))}
              </ul>
            </div>
          </div>
        </div>
      )}

      {/* Tabs */}
      <div className="border-b border-hone-200 dark:border-hone-700">
        <nav className="flex gap-4">
          {(["overview", "compare", "calls"] as const).map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={`px-1 py-3 text-sm font-medium border-b-2 transition-colors ${
                tab === t
                  ? "border-hone-700 dark:border-hone-300 text-hone-900 dark:text-hone-50"
                  : "border-transparent text-hone-500 hover:text-hone-700 dark:hover:text-hone-300"
              }`}
            >
              {t === "overview" ? "Overview" : t === "compare" ? "Model Comparison" : "Recent Calls"}
            </button>
          ))}
        </nav>
      </div>

      {/* Period Selector (for overview and compare) */}
      {(tab === "overview" || tab === "compare") && (
        <div className="flex items-center gap-2">
          <span className="text-sm text-hone-500">Period:</span>
          <select
            value={period}
            onChange={(e) => setPeriod(e.target.value)}
            className="input py-1 px-2 text-sm"
          >
            <option value="last-7-days">Last 7 days</option>
            <option value="last-30-days">Last 30 days</option>
            <option value="last-90-days">Last 90 days</option>
            <option value="all">All time</option>
          </select>
        </div>
      )}

      {/* Overview Tab */}
      {tab === "overview" && stats && (
        <div className="space-y-6">
          {/* Summary Stats */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <StatBox
              icon={<Activity className="w-5 h-5" />}
              label="Total Calls"
              value={stats.total_calls.toLocaleString()}
            />
            <StatBox
              icon={<CheckCircle className="w-5 h-5" />}
              label="Success Rate"
              value={`${(stats.success_rate * 100).toFixed(1)}%`}
              highlight={stats.success_rate >= 0.95}
              attention={stats.success_rate < 0.9}
            />
            <StatBox
              icon={<Clock className="w-5 h-5" />}
              label="Avg Latency"
              value={`${stats.avg_latency_ms.toFixed(0)}ms`}
              attention={stats.avg_latency_ms > 5000}
            />
            <StatBox
              icon={<Zap className="w-5 h-5" />}
              label="Est. Accuracy"
              value={`${(stats.accuracy.estimated_accuracy * 100).toFixed(1)}%`}
              highlight={stats.accuracy.estimated_accuracy >= 0.9}
              attention={stats.accuracy.estimated_accuracy < 0.85}
            />
          </div>

          {/* Latency Percentiles */}
          <div className="card">
            <div className="card-header">
              <h3 className="font-medium">Latency Distribution</h3>
            </div>
            <div className="p-4 grid grid-cols-3 gap-4 text-center">
              <div>
                <div className="text-2xl font-bold text-hone-700 dark:text-hone-300">
                  {stats.p50_latency_ms}ms
                </div>
                <div className="text-sm text-hone-500">p50 (median)</div>
              </div>
              <div>
                <div className="text-2xl font-bold text-hone-700 dark:text-hone-300">
                  {stats.p95_latency_ms}ms
                </div>
                <div className="text-sm text-hone-500">p95</div>
              </div>
              <div>
                <div className="text-2xl font-bold text-hone-700 dark:text-hone-300">
                  {stats.max_latency_ms}ms
                </div>
                <div className="text-sm text-hone-500">max</div>
              </div>
            </div>
          </div>

          {/* By Operation */}
          {stats.by_operation.length > 0 && (
            <div className="card">
              <div className="card-header">
                <h3 className="font-medium">By Operation</h3>
              </div>
              <div className="divide-y divide-hone-100 dark:divide-hone-700">
                {stats.by_operation.map((op) => (
                  <div key={op.operation} className="p-4 flex items-center justify-between">
                    <div>
                      <div className="font-medium">{formatOperation(op.operation)}</div>
                      <div className="text-sm text-hone-500">{op.call_count} calls</div>
                    </div>
                    <div className="text-right">
                      <div className="text-sm">
                        <span className={op.success_rate >= 0.95 ? "text-savings" : op.success_rate < 0.9 ? "text-waste" : ""}>
                          {(op.success_rate * 100).toFixed(1)}% success
                        </span>
                      </div>
                      <div className="text-sm text-hone-500">
                        {op.avg_latency_ms.toFixed(0)}ms avg
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Accuracy Stats */}
          <div className="card">
            <div className="card-header">
              <h3 className="font-medium">Accuracy (from corrections)</h3>
            </div>
            <div className="p-4">
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-center">
                <div>
                  <div className="text-xl font-bold">{stats.accuracy.total_ollama_tags}</div>
                  <div className="text-sm text-hone-500">Ollama tags</div>
                </div>
                <div>
                  <div className="text-xl font-bold">{stats.accuracy.total_corrections}</div>
                  <div className="text-sm text-hone-500">Corrections</div>
                </div>
                <div>
                  <div className="text-xl font-bold">
                    {(stats.accuracy.correction_rate * 100).toFixed(1)}%
                  </div>
                  <div className="text-sm text-hone-500">Correction rate</div>
                </div>
                <div>
                  <div className={`text-xl font-bold ${
                    stats.accuracy.estimated_accuracy >= 0.9 ? "text-savings" :
                    stats.accuracy.estimated_accuracy < 0.85 ? "text-waste" : ""
                  }`}>
                    {(stats.accuracy.estimated_accuracy * 100).toFixed(1)}%
                  </div>
                  <div className="text-sm text-hone-500">Est. accuracy</div>
                </div>
              </div>
              <p className="text-xs text-hone-400 mt-4">
                Accuracy is estimated from user corrections. When you manually change a tag
                that Ollama assigned, it&apos;s counted as a correction.
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Model Comparison Tab */}
      {tab === "compare" && (
        <div className="space-y-6">
          {comparisonLoading ? (
            <div className="flex items-center justify-center py-12">
              <RefreshCw className="w-5 h-5 animate-spin text-hone-400" />
              <span className="ml-2 text-hone-500">Loading model comparison...</span>
            </div>
          ) : comparisonStats && comparisonStats.models.length > 0 ? (
            <>
              {/* Summary comparison cards */}
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {comparisonStats.models.map((model) => (
                  <ModelCard
                    key={model.model}
                    model={model}
                    isExpanded={expandedModels.has(model.model)}
                    onToggle={() => {
                      const next = new Set(expandedModels);
                      if (next.has(model.model)) {
                        next.delete(model.model);
                      } else {
                        next.add(model.model);
                      }
                      setExpandedModels(next);
                    }}
                    allModels={comparisonStats.models}
                  />
                ))}
              </div>

              {/* Detailed comparison table */}
              <div className="card">
                <div className="card-header">
                  <h3 className="font-medium flex items-center gap-2">
                    <BarChart3 className="w-4 h-4" />
                    Side-by-Side Comparison
                  </h3>
                </div>
                <div className="overflow-x-auto">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="bg-hone-50 dark:bg-hone-800">
                        <th className="text-left p-3 font-medium">Metric</th>
                        {comparisonStats.models.map((m) => (
                          <th key={m.model} className="text-right p-3 font-medium">
                            {m.model}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-hone-100 dark:divide-hone-700">
                      <ComparisonRow
                        label="Total Calls"
                        values={comparisonStats.models.map((m) => m.total_calls)}
                        format={(v) => v !== null ? v.toLocaleString() : "N/A"}
                        higherIsBetter={true}
                      />
                      <ComparisonRow
                        label="Success Rate"
                        values={comparisonStats.models.map((m) => m.success_rate)}
                        format={(v) => v !== null ? `${(v * 100).toFixed(1)}%` : "N/A"}
                        higherIsBetter={true}
                      />
                      <ComparisonRow
                        label="Avg Latency"
                        values={comparisonStats.models.map((m) => m.avg_latency_ms)}
                        format={(v) => v !== null ? `${v.toFixed(0)}ms` : "N/A"}
                        higherIsBetter={false}
                      />
                      <ComparisonRow
                        label="p50 Latency"
                        values={comparisonStats.models.map((m) => m.p50_latency_ms)}
                        format={(v) => `${v}ms`}
                        higherIsBetter={false}
                      />
                      <ComparisonRow
                        label="p95 Latency"
                        values={comparisonStats.models.map((m) => m.p95_latency_ms)}
                        format={(v) => `${v}ms`}
                        higherIsBetter={false}
                      />
                      <ComparisonRow
                        label="Max Latency"
                        values={comparisonStats.models.map((m) => m.max_latency_ms)}
                        format={(v) => `${v}ms`}
                        higherIsBetter={false}
                      />
                      <ComparisonRow
                        label="Avg Confidence"
                        values={comparisonStats.models.map((m) => m.avg_confidence)}
                        format={(v) => v !== null ? `${(v * 100).toFixed(1)}%` : "N/A"}
                        higherIsBetter={true}
                      />
                    </tbody>
                  </table>
                </div>
              </div>

              {/* Per-operation breakdown */}
              <div className="card">
                <div className="card-header">
                  <h3 className="font-medium">Performance by Operation</h3>
                </div>
                <div className="p-4 space-y-4">
                  {getAllOperations(comparisonStats.models).map((op) => (
                    <div key={op} className="border border-hone-200 dark:border-hone-700 rounded-lg p-4">
                      <h4 className="font-medium mb-3">{formatOperation(op)}</h4>
                      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                        {comparisonStats.models.map((model) => {
                          const opStats = model.by_operation.find((o) => o.operation === op);
                          if (!opStats) return null;
                          return (
                            <div
                              key={model.model}
                              className="bg-hone-50 dark:bg-hone-800 rounded p-3"
                            >
                              <div className="font-medium text-sm mb-2">{model.model}</div>
                              <div className="text-xs space-y-1 text-hone-600 dark:text-hone-400">
                                <div className="flex justify-between">
                                  <span>Calls:</span>
                                  <span>{opStats.call_count}</span>
                                </div>
                                <div className="flex justify-between">
                                  <span>Success:</span>
                                  <span className={opStats.success_rate >= 0.95 ? "text-savings" : opStats.success_rate < 0.9 ? "text-waste" : ""}>
                                    {(opStats.success_rate * 100).toFixed(1)}%
                                  </span>
                                </div>
                                <div className="flex justify-between">
                                  <span>Latency:</span>
                                  <span>{opStats.avg_latency_ms.toFixed(0)}ms</span>
                                </div>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </>
          ) : (
            <div className="card p-8 text-center">
              <BarChart3 className="w-12 h-12 text-hone-300 mx-auto mb-4" />
              <h3 className="font-medium mb-2">No Models to Compare</h3>
              <p className="text-sm text-hone-500">
                Use different models to see comparison data. Change your OLLAMA_MODEL
                environment variable and run some imports to collect data.
              </p>
            </div>
          )}
        </div>
      )}

      {/* Calls Tab */}
      {tab === "calls" && (
        <div className="card">
          <div className="card-header flex items-center justify-between">
            <h3 className="font-medium">Recent Calls</h3>
            <div className="flex items-center gap-3">
              <select
                value={operationFilter}
                onChange={(e) => setOperationFilter(e.target.value)}
                className="input py-1 px-2 text-sm"
              >
                <option value="all">All operations</option>
                {[...new Set(calls.map((c) => c.operation))].map((op) => (
                  <option key={op} value={op}>
                    {formatOperation(op)}
                  </option>
                ))}
              </select>
              <select
                value={callsFilter}
                onChange={(e) => setCallsFilter(e.target.value as "all" | "failures")}
                className="input py-1 px-2 text-sm"
              >
                <option value="all">All calls</option>
                <option value="failures">Failures only</option>
              </select>
            </div>
          </div>
          {(() => {
            const filteredCalls = calls.filter((call) => {
              if (callsFilter === "failures" && call.success) return false;
              if (operationFilter !== "all" && call.operation !== operationFilter) return false;
              return true;
            });

            if (filteredCalls.length === 0) {
              return (
                <div className="p-8 text-center text-hone-500">
                  <Server className="w-8 h-8 mx-auto mb-2 opacity-50" />
                  {calls.length === 0 ? "No AI calls recorded yet" : "No calls match the current filters"}
                </div>
              );
            }

            return (
              <>
                <div className="px-4 py-2 bg-hone-50 dark:bg-hone-800/50 text-sm text-hone-500 border-b border-hone-200 dark:border-hone-700 flex items-center justify-between">
                  <span>
                    Showing {filteredCalls.length} of {calls.length} calls
                    {callsFilter === "failures" && ` (${calls.filter(c => !c.success).length} failures)`}
                  </span>
                  {hasMoreCalls && (
                    <button
                      onClick={loadMoreCalls}
                      disabled={loadingMore}
                      className="text-hone-600 dark:text-hone-400 hover:text-hone-800 dark:hover:text-hone-200 disabled:opacity-50"
                    >
                      {loadingMore ? (
                        <span className="flex items-center gap-1">
                          <RefreshCw className="w-3 h-3 animate-spin" />
                          Loading...
                        </span>
                      ) : (
                        "Load more"
                      )}
                    </button>
                  )}
                </div>
                <div className="divide-y divide-hone-100 dark:divide-hone-700 max-h-[600px] overflow-y-auto">
                  {filteredCalls.map((call) => {
                const isExpanded = expandedCalls.has(call.id);
                const hasDetails = call.input_text || call.result_text || call.error_message || call.metadata;
                const toggleExpanded = () => {
                  const next = new Set(expandedCalls);
                  if (isExpanded) {
                    next.delete(call.id);
                  } else {
                    next.add(call.id);
                  }
                  setExpandedCalls(next);
                };

                return (
                  <div key={call.id}>
                    <div
                      className={`p-3 flex items-center gap-3 text-sm ${hasDetails ? "cursor-pointer hover:bg-hone-100 dark:hover:bg-hone-700" : ""}`}
                      onClick={hasDetails ? toggleExpanded : undefined}
                    >
                      {hasDetails && (
                        <span className="text-hone-400 flex-shrink-0">
                          {isExpanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
                        </span>
                      )}
                      {call.success ? (
                        <CheckCircle className="w-4 h-4 text-savings flex-shrink-0" />
                      ) : (
                        <XCircle className="w-4 h-4 text-waste flex-shrink-0" />
                      )}
                      <div className="flex-1 min-w-0">
                        <div className="font-medium">{formatOperation(call.operation)}</div>
                        <div className="text-xs text-hone-500 truncate">
                          {new Date(call.started_at).toLocaleString()}
                          <span className="ml-2 text-hone-400">[{call.model}]</span>
                          {call.input_text && !isExpanded && (
                            <span className="ml-2 text-hone-400">"{call.input_text}"</span>
                          )}
                        </div>
                      </div>
                      <div className="text-right flex-shrink-0">
                        <div className="font-mono">{call.latency_ms}ms</div>
                        {call.confidence !== null && (
                          <div className="text-xs text-hone-500">
                            {(call.confidence * 100).toFixed(0)}% conf
                          </div>
                        )}
                      </div>
                    </div>
                    {isExpanded && (
                      <div className="px-4 pb-3 ml-8 text-sm space-y-2 bg-hone-50/50 dark:bg-hone-800/50">
                        {call.input_text && (
                          <div>
                            <span className="text-hone-500">Input:</span>{" "}
                            <span className="font-mono text-hone-700 dark:text-hone-300">{call.input_text}</span>
                          </div>
                        )}
                        {call.result_text && (
                          <div>
                            <span className="text-hone-500">Result:</span>{" "}
                            <span className="font-mono text-savings">{call.result_text}</span>
                          </div>
                        )}
                        {call.error_message && (
                          <div>
                            <span className="text-hone-500">Error:</span>{" "}
                            <span className="text-waste">{call.error_message}</span>
                          </div>
                        )}
                        {call.metadata && (() => {
                          try {
                            const meta = JSON.parse(call.metadata);
                            const toolCalls = meta.tool_calls as ToolCallRecord[] | undefined;
                            if (toolCalls && toolCalls.length > 0) {
                              return (
                                <div className="mt-2 pt-2 border-t border-hone-200 dark:border-hone-700">
                                  <div className="text-hone-500 mb-1">Tool Calls ({toolCalls.length}):</div>
                                  <div className="space-y-2">
                                    {toolCalls.map((tc, idx) => (
                                      <div key={idx} className="bg-hone-100 dark:bg-hone-700 rounded p-2">
                                        <div className="flex items-center gap-2">
                                          {tc.success ? (
                                            <CheckCircle className="w-3 h-3 text-savings flex-shrink-0" />
                                          ) : (
                                            <XCircle className="w-3 h-3 text-waste flex-shrink-0" />
                                          )}
                                          <span className="font-mono font-medium">{tc.name}</span>
                                        </div>
                                        <div className="mt-1 text-xs">
                                          <span className="text-hone-500">Input:</span>{" "}
                                          <span className="font-mono text-hone-600 dark:text-hone-400">
                                            {JSON.stringify(tc.input)}
                                          </span>
                                        </div>
                                        {tc.output && (
                                          <div className="mt-1 text-xs">
                                            <span className="text-hone-500">Output:</span>{" "}
                                            <span className="font-mono text-hone-600 dark:text-hone-400 break-all">
                                              {tc.output.length > 200 ? tc.output.substring(0, 200) + "..." : tc.output}
                                            </span>
                                          </div>
                                        )}
                                      </div>
                                    ))}
                                  </div>
                                  {meta.iterations && (
                                    <div className="text-xs text-hone-400 mt-2">
                                      Iterations: {meta.iterations}
                                    </div>
                                  )}
                                </div>
                              );
                            }
                            return null;
                          } catch {
                            return null;
                          }
                        })()}
                        <div className="text-xs text-hone-400">
                          Model: {call.model} • ID: {call.id}
                        </div>
                      </div>
                    )}
                  </div>
                );
              })}
                </div>
              </>
            );
          })()}
        </div>
      )}
    </div>
  );
}

function StatBox({
  icon,
  label,
  value,
  highlight,
  attention,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  highlight?: boolean;
  attention?: boolean;
}) {
  return (
    <div className={`card p-4 ${highlight ? "ring-1 ring-savings/30" : attention ? "ring-1 ring-attention/30" : ""}`}>
      <div className={`mb-2 ${highlight ? "text-savings" : attention ? "text-attention" : "text-hone-400"}`}>
        {icon}
      </div>
      <div className="text-sm text-hone-500">{label}</div>
      <div className={`text-xl font-bold ${highlight ? "text-savings" : attention ? "text-attention" : ""}`}>
        {value}
      </div>
    </div>
  );
}

function formatOperation(op: string): string {
  return op
    .split("_")
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(" ");
}

// Model comparison components

function ModelCard({
  model,
  isExpanded,
  onToggle,
  allModels,
}: {
  model: ModelStats;
  isExpanded: boolean;
  onToggle: () => void;
  allModels: ModelStats[];
}) {
  // Determine if this model is the best for key metrics
  const isBestLatency = allModels.every((m) => m.avg_latency_ms >= model.avg_latency_ms);
  const isBestSuccess = allModels.every((m) => m.success_rate <= model.success_rate);
  const hasMostCalls = allModels.every((m) => m.total_calls <= model.total_calls);

  return (
    <div className="card">
      <div
        className="p-4 cursor-pointer hover:bg-hone-50 dark:hover:bg-hone-800"
        onClick={onToggle}
      >
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-2">
            <Cpu className="w-5 h-5 text-hone-400" />
            <h3 className="font-semibold">{model.model}</h3>
          </div>
          <span className="text-hone-400">
            {isExpanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
          </span>
        </div>

        <div className="mt-3 grid grid-cols-3 gap-2 text-center">
          <div>
            <div className="text-lg font-bold">{model.total_calls.toLocaleString()}</div>
            <div className="text-xs text-hone-500">
              calls {hasMostCalls && allModels.length > 1 && <span className="text-savings">★</span>}
            </div>
          </div>
          <div>
            <div className={`text-lg font-bold ${model.success_rate >= 0.95 ? "text-savings" : model.success_rate < 0.9 ? "text-waste" : ""}`}>
              {(model.success_rate * 100).toFixed(1)}%
            </div>
            <div className="text-xs text-hone-500">
              success {isBestSuccess && allModels.length > 1 && <span className="text-savings">★</span>}
            </div>
          </div>
          <div>
            <div className="text-lg font-bold">{model.avg_latency_ms.toFixed(0)}ms</div>
            <div className="text-xs text-hone-500">
              avg {isBestLatency && allModels.length > 1 && <span className="text-savings">★</span>}
            </div>
          </div>
        </div>
      </div>

      {isExpanded && (
        <div className="border-t border-hone-200 dark:border-hone-700 p-4 bg-hone-50/50 dark:bg-hone-800/50">
          <div className="space-y-3 text-sm">
            <div className="grid grid-cols-3 gap-2 text-center">
              <div>
                <div className="font-medium">{model.p50_latency_ms}ms</div>
                <div className="text-xs text-hone-500">p50</div>
              </div>
              <div>
                <div className="font-medium">{model.p95_latency_ms}ms</div>
                <div className="text-xs text-hone-500">p95</div>
              </div>
              <div>
                <div className="font-medium">{model.max_latency_ms}ms</div>
                <div className="text-xs text-hone-500">max</div>
              </div>
            </div>

            {model.avg_confidence !== null && (
              <div className="text-center">
                <span className="text-hone-500">Avg Confidence:</span>{" "}
                <span className="font-medium">{(model.avg_confidence * 100).toFixed(1)}%</span>
              </div>
            )}

            {model.first_used && (
              <div className="text-xs text-hone-400 text-center">
                Used: {new Date(model.first_used).toLocaleDateString()} - {model.last_used ? new Date(model.last_used).toLocaleDateString() : "now"}
              </div>
            )}

            {model.by_operation.length > 0 && (
              <div className="mt-3 pt-3 border-t border-hone-200 dark:border-hone-700">
                <div className="text-xs font-medium text-hone-500 mb-2">Operations:</div>
                <div className="space-y-1">
                  {model.by_operation.map((op) => (
                    <div key={op.operation} className="flex justify-between text-xs">
                      <span>{formatOperation(op.operation)}</span>
                      <span className="text-hone-500">
                        {op.call_count} calls, {op.avg_latency_ms.toFixed(0)}ms
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function ComparisonRow({
  label,
  values,
  format,
  higherIsBetter,
}: {
  label: string;
  values: (number | null)[];
  format: (v: number | null) => string;
  higherIsBetter: boolean;
}) {
  // Find best value (ignoring nulls)
  const numericValues = values.filter((v): v is number => v !== null);
  const bestValue = numericValues.length > 0
    ? (higherIsBetter ? Math.max(...numericValues) : Math.min(...numericValues))
    : null;

  return (
    <tr className="hover:bg-hone-50 dark:hover:bg-hone-800/50">
      <td className="p-3 font-medium">{label}</td>
      {values.map((value, i) => {
        const isBest = value !== null && bestValue !== null && value === bestValue && numericValues.length > 1;
        return (
          <td key={i} className="p-3 text-right">
            <span className={`${isBest ? "text-savings font-semibold" : ""}`}>
              {format(value)}
              {isBest && (
                <span className="ml-1 inline-flex items-center">
                  {higherIsBetter ? (
                    <ArrowUpRight className="w-3 h-3" />
                  ) : (
                    <ArrowDownRight className="w-3 h-3" />
                  )}
                </span>
              )}
            </span>
          </td>
        );
      })}
    </tr>
  );
}

function getAllOperations(models: ModelStats[]): string[] {
  const ops = new Set<string>();
  for (const model of models) {
    for (const op of model.by_operation) {
      ops.add(op.operation);
    }
  }
  return Array.from(ops).sort();
}
