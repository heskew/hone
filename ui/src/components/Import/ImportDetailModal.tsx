import {
  AlertTriangle,
  ArrowRight,
  Ban,
  Calendar,
  CheckCircle,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Clock,
  Copy,
  CreditCard,
  FileText,
  Ghost,
  GitCompare,
  History,
  Loader2,
  RefreshCw,
  Settings,
  Tag,
  TrendingUp,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api";
import type {
  ImportSessionWithAccount,
  ReprocessComparison,
  ReprocessRunSummary,
  RunComparison,
  SkippedTransaction,
  Transaction,
} from "../../types";

interface ImportDetailModalProps {
  session: ImportSessionWithAccount;
  onClose: () => void;
  onReprocessed?: () => void;
  onSessionRefresh?: () => void;
}

type TabType = "overview" | "transactions" | "skipped" | "tagging" | "comparison" | "history";

export function ImportDetailModal({ session, onClose, onReprocessed, onSessionRefresh }: ImportDetailModalProps) {
  const [activeTab, setActiveTab] = useState<TabType>("overview");
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [transactionsTotal, setTransactionsTotal] = useState(0);
  const [skipped, setSkipped] = useState<SkippedTransaction[]>([]);
  const [loading, setLoading] = useState(false);
  const [reprocessing, setReprocessing] = useState(false);
  const [comparison, setComparison] = useState<ReprocessComparison | null>(null);
  const [comparisonLoading, setComparisonLoading] = useState(false);
  const [txPage, setTxPage] = useState(0);
  const txPageSize = 20;

  // Historical comparison state
  const [runs, setRuns] = useState<ReprocessRunSummary[]>([]);
  const [runsLoading, setRunsLoading] = useState(false);
  const [selectedRunA, setSelectedRunA] = useState<number | null>(null);
  const [selectedRunB, setSelectedRunB] = useState<number | null>(null);
  const [runComparison, setRunComparison] = useState<RunComparison | null>(null);
  const [runComparisonLoading, setRunComparisonLoading] = useState(false);

  // Model selection state for reprocess
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [defaultModel, setDefaultModel] = useState<string>("");
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [showModelSelector, setShowModelSelector] = useState(false);
  const [loadingModels, setLoadingModels] = useState(false);

  // Cancel state
  const [cancelling, setCancelling] = useState(false);

  const s = session.session;

  // Load available models on mount, default to what was used in the import
  useEffect(() => {
    setLoadingModels(true);
    api.getExploreModels()
      .then((response) => {
        setAvailableModels(response.models);
        setDefaultModel(response.default_model);
        // Default to the model used in this import, or fall back to server default
        const importModel = s.ollama_model;
        if (importModel && response.models.includes(importModel)) {
          setSelectedModel(importModel);
        } else {
          setSelectedModel(response.default_model);
        }
      })
      .catch((err) => {
        console.error("Failed to load models:", err);
      })
      .finally(() => setLoadingModels(false));
  }, [s.ollama_model]);

  // Map technical phase names to user-friendly descriptions
  const getPhaseDescription = (phase: string | null): string => {
    if (!phase) return "Starting...";
    switch (phase) {
      case "clearing":
        return "Clearing previous results";
      case "tagging":
        return "Categorizing transactions";
      case "normalizing":
        return "Extracting merchant names";
      case "matching_receipts":
        return "Matching receipts";
      case "detecting":
        return "Detecting subscriptions & alerts";
      case "classifying_merchants":
        return "Classifying merchants";
      case "analyzing_duplicates":
        return "Analyzing duplicate subscriptions";
      case "analyzing_spending":
        return "Analyzing spending patterns";
      default:
        return phase;
    }
  };

  // When session status changes from processing to completed, reload comparison and runs
  useEffect(() => {
    if (s.status === "completed" && reprocessing) {
      setReprocessing(false);
      // Load comparison data after reprocess completes
      loadComparison();
      // Also reload runs to get updated status
      loadRuns();
    }
  }, [s.status]);

  // Load comparison when switching to comparison tab
  useEffect(() => {
    if (activeTab === "comparison" && comparison === null && s.status === "completed") {
      loadComparison();
    }
  }, [activeTab]);

  // Load runs when switching to history tab OR when session status/phase changes
  useEffect(() => {
    if (activeTab === "history") {
      loadRuns();
    }
  }, [activeTab, s.status, s.processing_phase]);

  // Load run comparison when both runs are selected
  useEffect(() => {
    if (selectedRunA !== null && selectedRunB !== null && selectedRunA !== selectedRunB) {
      loadRunComparison();
    } else {
      setRunComparison(null);
    }
  }, [selectedRunA, selectedRunB]);

  // Close on Escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  // Load transactions when switching to transactions tab
  useEffect(() => {
    if (activeTab === "transactions") {
      loadTransactions();
    }
  }, [activeTab, txPage]);

  // Load skipped when switching to skipped tab
  useEffect(() => {
    if (activeTab === "skipped" && skipped.length === 0 && s.skipped_count > 0) {
      loadSkipped();
    }
  }, [activeTab]);

  const loadTransactions = async () => {
    try {
      setLoading(true);
      const response = await api.getImportSessionTransactions(s.id, {
        limit: txPageSize,
        offset: txPage * txPageSize,
      });
      setTransactions(response.transactions);
      setTransactionsTotal(response.total);
    } catch (err) {
      console.error("Failed to load transactions:", err);
    } finally {
      setLoading(false);
    }
  };

  const loadSkipped = async () => {
    try {
      setLoading(true);
      const result = await api.getImportSessionSkipped(s.id);
      setSkipped(result);
    } catch (err) {
      console.error("Failed to load skipped transactions:", err);
    } finally {
      setLoading(false);
    }
  };

  const handleReprocess = async () => {
    try {
      setReprocessing(true);
      setComparison(null);
      // Use model override if different from default
      const modelToUse = selectedModel !== defaultModel ? selectedModel : undefined;
      await api.reprocessImportSession(s.id, modelToUse);
      // Reprocess started in background, notify parent to start polling
      onReprocessed?.();
      onSessionRefresh?.();
    } catch (err) {
      console.error("Failed to start reprocess:", err);
      setReprocessing(false);
    }
  };

  const handleCancel = async () => {
    try {
      setCancelling(true);
      const result = await api.cancelImportSession(s.id);
      if (result.cancelled) {
        // Refresh session to show cancelled status
        onSessionRefresh?.();
      }
    } catch (err) {
      console.error("Failed to cancel import:", err);
    } finally {
      setCancelling(false);
    }
  };

  const loadComparison = async () => {
    try {
      setComparisonLoading(true);
      const result = await api.getReprocessComparison(s.id);
      setComparison(result);
    } catch (err) {
      console.error("Failed to load comparison:", err);
    } finally {
      setComparisonLoading(false);
    }
  };

  const loadRuns = async () => {
    try {
      setRunsLoading(true);
      const result = await api.getReprocessRuns(s.id);

      // Add synthetic "Initial Import" entry (id=0, run_number=0)
      // This allows comparing any run back to the original import baseline
      const initialImportRun: ReprocessRunSummary = {
        id: 0,
        run_number: 0,
        ollama_model: s.ollama_model || null,
        status: "completed",
        initiated_by: s.user_email || null,
        started_at: s.created_at,
        completed_at: s.created_at,
        tags_changed: 0,
        merchants_changed: 0,
      };

      // Prepend initial import to the list (shown at top since run_number=0)
      const runsWithInitial = [...result, initialImportRun].sort(
        (a, b) => a.run_number - b.run_number
      );
      setRuns(runsWithInitial);

      // Auto-select: initial import (A) vs most recent run (B)
      if (result.length >= 1) {
        setSelectedRunA(0); // Initial import
        setSelectedRunB(result[0].id); // Most recent reprocess
      }
    } catch (err) {
      console.error("Failed to load runs:", err);
    } finally {
      setRunsLoading(false);
    }
  };

  const loadRunComparison = async () => {
    if (selectedRunA === null || selectedRunB === null) return;
    try {
      setRunComparisonLoading(true);
      const result = await api.compareReprocessRuns(s.id, selectedRunA, selectedRunB);
      setRunComparison(result);
    } catch (err) {
      console.error("Failed to load run comparison:", err);
      setRunComparison(null);
    } finally {
      setRunComparisonLoading(false);
    }
  };

  // Parse date-only strings as local dates to avoid timezone shift
  const formatDate = (dateStr: string) => {
    const [year, month, day] = dateStr.split("-").map(Number);
    return new Date(year, month - 1, day, 12, 0, 0).toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
    });
  };

  const formatDateTime = (dateStr: string) => {
    return new Date(dateStr).toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  };

  const formatFileSize = (bytes: number | null) => {
    if (bytes === null) return "-";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatDuration = (ms: number | null) => {
    if (ms === null) return "-";
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
    const minutes = Math.floor(ms / 60000);
    const seconds = ((ms % 60000) / 1000).toFixed(0);
    return `${minutes}m ${seconds}s`;
  };

  const tabClass = (tab: TabType) =>
    `px-3 py-2 text-sm font-medium border-b-2 transition-colors ${
      activeTab === tab
        ? "border-hone-700 text-hone-900 dark:text-hone-100"
        : "border-transparent text-hone-500 hover:text-hone-700 dark:text-hone-400 dark:hover:text-hone-200"
    }`;

  const totalTagged =
    s.tagged_by_learned + s.tagged_by_rule + s.tagged_by_pattern + s.tagged_by_ollama + s.tagged_by_bank_category + s.tagged_fallback;
  const txTotalPages = Math.ceil(transactionsTotal / txPageSize);

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      onClick={onClose}
    >
      <div
        className="card w-full max-w-3xl mx-4 max-h-[90vh] flex flex-col animate-slide-up"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="card-header flex items-start justify-between">
          <div>
            <h2 className="text-lg font-semibold text-hone-900 dark:text-hone-100">
              Import Details
            </h2>
            <div className="flex items-center gap-3 mt-1 text-sm text-hone-500 dark:text-hone-400">
              <span className="flex items-center gap-1">
                <Calendar className="w-4 h-4" />
                {formatDateTime(s.created_at)}
              </span>
              <span className="flex items-center gap-1">
                <CreditCard className="w-4 h-4" />
                {session.account_name}
              </span>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200 rounded"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Tabs */}
        <div className="border-b border-hone-200 dark:border-hone-700 px-4">
          <div className="flex gap-4">
            <button className={tabClass("overview")} onClick={() => setActiveTab("overview")}>
              Overview
            </button>
            <button className={tabClass("transactions")} onClick={() => setActiveTab("transactions")}>
              Transactions
              {s.imported_count > 0 && (
                <span className="ml-1.5 px-1.5 py-0.5 text-xs rounded-full bg-savings/20 text-savings">
                  {s.imported_count}
                </span>
              )}
            </button>
            <button className={tabClass("skipped")} onClick={() => setActiveTab("skipped")}>
              Skipped
              {s.skipped_count > 0 && (
                <span className="ml-1.5 px-1.5 py-0.5 text-xs rounded-full bg-attention/20 text-attention">
                  {s.skipped_count}
                </span>
              )}
            </button>
            <button className={tabClass("tagging")} onClick={() => setActiveTab("tagging")}>
              Tagging
            </button>
            {comparison && (
              <button className={tabClass("comparison")} onClick={() => setActiveTab("comparison")}>
                <GitCompare className="w-3.5 h-3.5 inline mr-1" />
                Comparison
              </button>
            )}
            <button className={tabClass("history")} onClick={() => setActiveTab("history")}>
              <History className="w-3.5 h-3.5 inline mr-1" />
              History
              {runs.length > 0 && (
                <span className="ml-1.5 px-1.5 py-0.5 text-xs rounded-full bg-hone-200 dark:bg-hone-700 text-hone-600 dark:text-hone-300">
                  {runs.length}
                </span>
              )}
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="card-body overflow-y-auto flex-1">
          {activeTab === "overview" && (
            <div className="space-y-6">
              {/* Processing Status Banner */}
              {(s.status === "pending" || s.status === "processing") && (
                <div className="bg-hone-100 dark:bg-hone-800 rounded-lg p-4 flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <RefreshCw className="w-5 h-5 animate-spin text-hone-600 dark:text-hone-400" />
                    <div>
                      <div className="font-medium text-hone-900 dark:text-hone-100">
                        Processing in background...
                      </div>
                      <div className="text-sm text-hone-600 dark:text-hone-400">
                        {getPhaseDescription(s.processing_phase)}{" "}
                        {s.processing_total > 0 && `(${s.processing_current} of ${s.processing_total})`}
                      </div>
                    </div>
                  </div>
                  <button
                    onClick={handleCancel}
                    disabled={cancelling}
                    className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-lg
                              bg-hone-200 dark:bg-hone-700 text-hone-700 dark:text-hone-300
                              hover:bg-hone-300 dark:hover:bg-hone-600
                              disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                  >
                    {cancelling ? (
                      <Loader2 className="w-4 h-4 animate-spin" />
                    ) : (
                      <Ban className="w-4 h-4" />
                    )}
                    Cancel
                  </button>
                </div>
              )}
              {s.status === "failed" && (
                <div className="bg-waste/10 rounded-lg p-4 flex items-center gap-3">
                  <AlertTriangle className="w-5 h-5 text-waste" />
                  <div>
                    <div className="font-medium text-waste">Processing Failed</div>
                    <div className="text-sm text-hone-600 dark:text-hone-400">
                      {s.processing_error || "An error occurred during processing"}
                    </div>
                  </div>
                </div>
              )}
              {s.status === "cancelled" && (
                <div className="bg-attention/10 rounded-lg p-4 flex items-center gap-3">
                  <Ban className="w-5 h-5 text-attention" />
                  <div>
                    <div className="font-medium text-attention">Import Cancelled</div>
                    <div className="text-sm text-hone-600 dark:text-hone-400">
                      {s.processing_error || "Import was cancelled by user"}
                    </div>
                  </div>
                </div>
              )}

              {/* Summary Stats */}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-4">
                  <div className="text-2xl font-bold text-savings">{s.imported_count}</div>
                  <div className="text-sm text-hone-500 dark:text-hone-400">Imported</div>
                </div>
                <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-4">
                  <div className="text-2xl font-bold text-attention">{s.skipped_count}</div>
                  <div className="text-sm text-hone-500 dark:text-hone-400">Skipped (dupes)</div>
                </div>
                <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-4">
                  <div className="text-2xl font-bold text-hone-700 dark:text-hone-300">{totalTagged}</div>
                  <div className="text-sm text-hone-500 dark:text-hone-400">Tagged</div>
                </div>
                <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-4">
                  <div className="text-2xl font-bold text-hone-700 dark:text-hone-300">{s.receipts_matched}</div>
                  <div className="text-sm text-hone-500 dark:text-hone-400">Receipts Matched</div>
                </div>
              </div>

              {/* Detection Results */}
              {(s.subscriptions_found > 0 || s.zombies_detected > 0 || s.price_increases_detected > 0 || s.duplicates_detected > 0) && (
                <div>
                  <h3 className="text-sm font-medium text-hone-700 dark:text-hone-300 mb-3">
                    Detection Results
                  </h3>
                  <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                    {s.subscriptions_found > 0 && (
                      <a
                        href="#/subscriptions"
                        className="flex items-center gap-2 text-sm hover:text-hone-600 dark:hover:text-hone-200 transition-colors"
                        onClick={onClose}
                      >
                        <CheckCircle className="w-4 h-4 text-savings" />
                        <span className="hover:underline">{s.subscriptions_found} subscriptions found</span>
                      </a>
                    )}
                    {s.zombies_detected > 0 && (
                      <a
                        href="#/alerts"
                        className="flex items-center gap-2 text-sm hover:text-hone-600 dark:hover:text-hone-200 transition-colors"
                        onClick={onClose}
                      >
                        <Ghost className="w-4 h-4 text-attention" />
                        <span className="hover:underline">{s.zombies_detected} zombies</span>
                      </a>
                    )}
                    {s.price_increases_detected > 0 && (
                      <a
                        href="#/alerts"
                        className="flex items-center gap-2 text-sm hover:text-hone-600 dark:hover:text-hone-200 transition-colors"
                        onClick={onClose}
                      >
                        <TrendingUp className="w-4 h-4 text-waste" />
                        <span className="hover:underline">{s.price_increases_detected} price increases</span>
                      </a>
                    )}
                    {s.duplicates_detected > 0 && (
                      <a
                        href="#/alerts"
                        className="flex items-center gap-2 text-sm hover:text-hone-600 dark:hover:text-hone-200 transition-colors"
                        onClick={onClose}
                      >
                        <Copy className="w-4 h-4 text-attention" />
                        <span className="hover:underline">{s.duplicates_detected} duplicate services</span>
                      </a>
                    )}
                  </div>
                </div>
              )}

              {/* File Info */}
              <div>
                <h3 className="text-sm font-medium text-hone-700 dark:text-hone-300 mb-3">
                  File Information
                </h3>
                <div className="grid grid-cols-2 gap-4 text-sm">
                  <div>
                    <span className="text-hone-500 dark:text-hone-400">Bank Format: </span>
                    <span className="text-hone-900 dark:text-hone-100 uppercase">{s.bank}</span>
                  </div>
                  <div>
                    <span className="text-hone-500 dark:text-hone-400">File Size: </span>
                    <span className="text-hone-900 dark:text-hone-100">{formatFileSize(s.file_size_bytes)}</span>
                  </div>
                  {s.filename && (
                    <div className="col-span-2">
                      <span className="text-hone-500 dark:text-hone-400">Filename: </span>
                      <span className="text-hone-900 dark:text-hone-100">{s.filename}</span>
                    </div>
                  )}
                  {s.user_email && (
                    <div className="col-span-2">
                      <span className="text-hone-500 dark:text-hone-400">Imported by: </span>
                      <span className="text-hone-900 dark:text-hone-100">{s.user_email}</span>
                    </div>
                  )}
                  {s.ollama_model && (
                    <div className="col-span-2">
                      <span className="text-hone-500 dark:text-hone-400">Ollama Model: </span>
                      <span className="text-hone-900 dark:text-hone-100">{s.ollama_model}</span>
                    </div>
                  )}
                </div>
              </div>

              {/* Processing Timing */}
              {s.total_duration_ms !== null && (
                <div>
                  <h3 className="text-sm font-medium text-hone-700 dark:text-hone-300 mb-3 flex items-center gap-2">
                    <Clock className="w-4 h-4" />
                    Processing Time
                  </h3>
                  <div className="space-y-2">
                    {/* Phase breakdown */}
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                      {s.tagging_duration_ms !== null && (
                        <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-3">
                          <div className="text-lg font-semibold text-hone-900 dark:text-hone-100">
                            {formatDuration(s.tagging_duration_ms)}
                          </div>
                          <div className="text-xs text-hone-500 dark:text-hone-400">Categorizing</div>
                        </div>
                      )}
                      {s.normalizing_duration_ms !== null && (
                        <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-3">
                          <div className="text-lg font-semibold text-hone-900 dark:text-hone-100">
                            {formatDuration(s.normalizing_duration_ms)}
                          </div>
                          <div className="text-xs text-hone-500 dark:text-hone-400">Normalizing</div>
                        </div>
                      )}
                      {s.matching_duration_ms !== null && (
                        <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-3">
                          <div className="text-lg font-semibold text-hone-900 dark:text-hone-100">
                            {formatDuration(s.matching_duration_ms)}
                          </div>
                          <div className="text-xs text-hone-500 dark:text-hone-400">Matching</div>
                        </div>
                      )}
                      {s.detecting_duration_ms !== null && (
                        <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-3">
                          <div className="text-lg font-semibold text-hone-900 dark:text-hone-100">
                            {formatDuration(s.detecting_duration_ms)}
                          </div>
                          <div className="text-xs text-hone-500 dark:text-hone-400">Detecting</div>
                        </div>
                      )}
                    </div>
                    {/* Total */}
                    <div className="text-sm text-hone-600 dark:text-hone-400 pt-2 border-t border-hone-200 dark:border-hone-700">
                      Total processing time: <span className="font-medium text-hone-900 dark:text-hone-100">{formatDuration(s.total_duration_ms)}</span>
                    </div>
                  </div>
                </div>
              )}
            </div>
          )}

          {activeTab === "transactions" && (
            <div>
              {loading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 className="w-5 h-5 animate-spin text-hone-400" />
                </div>
              ) : transactions.length === 0 ? (
                <div className="text-center py-8 text-hone-500 dark:text-hone-400">
                  No transactions in this import
                </div>
              ) : (
                <>
                  <div className="divide-y divide-hone-100 dark:divide-hone-800">
                    {transactions.map((tx) => (
                      <div
                        key={tx.id}
                        className="py-3 flex items-center justify-between hover:bg-hone-50 dark:hover:bg-hone-800/50 -mx-4 px-4"
                      >
                        <div className="flex-1 min-w-0">
                          <div className="text-sm font-medium text-hone-900 dark:text-hone-100 truncate">
                            {tx.merchant_normalized || tx.description}
                          </div>
                          <div className="flex items-center gap-2 mt-0.5">
                            <span className="text-xs text-hone-500 dark:text-hone-400">
                              {formatDate(tx.date)}
                            </span>
                            {tx.merchant_normalized && tx.merchant_normalized !== tx.description && (
                              <span className="text-xs text-hone-400 truncate max-w-[150px]">
                                {tx.description}
                              </span>
                            )}
                          </div>
                        </div>
                        <div className={`font-medium ${tx.amount < 0 ? "amount-negative" : "amount-positive"}`}>
                          ${Math.abs(tx.amount).toFixed(2)}
                        </div>
                      </div>
                    ))}
                  </div>

                  {/* Pagination */}
                  {txTotalPages > 1 && (
                    <div className="flex items-center justify-between mt-4 pt-4 border-t border-hone-200 dark:border-hone-700">
                      <div className="text-sm text-hone-500 dark:text-hone-400">
                        {txPage * txPageSize + 1}-{Math.min((txPage + 1) * txPageSize, transactionsTotal)} of {transactionsTotal}
                      </div>
                      <div className="flex items-center gap-2">
                        <button
                          onClick={() => setTxPage((p) => Math.max(0, p - 1))}
                          disabled={txPage === 0}
                          className="p-1.5 rounded hover:bg-hone-100 dark:hover:bg-hone-800 disabled:opacity-30"
                        >
                          <ChevronLeft className="w-4 h-4" />
                        </button>
                        <span className="text-sm text-hone-600 dark:text-hone-400">
                          {txPage + 1} / {txTotalPages}
                        </span>
                        <button
                          onClick={() => setTxPage((p) => Math.min(txTotalPages - 1, p + 1))}
                          disabled={txPage >= txTotalPages - 1}
                          className="p-1.5 rounded hover:bg-hone-100 dark:hover:bg-hone-800 disabled:opacity-30"
                        >
                          <ChevronRight className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  )}
                </>
              )}
            </div>
          )}

          {activeTab === "skipped" && (
            <div>
              {loading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 className="w-5 h-5 animate-spin text-hone-400" />
                </div>
              ) : skipped.length === 0 ? (
                <div className="text-center py-8 text-hone-500 dark:text-hone-400">
                  <AlertTriangle className="w-8 h-8 mx-auto mb-2 opacity-30" />
                  <p>No duplicates were skipped in this import</p>
                </div>
              ) : (
                <div className="space-y-1">
                  <p className="text-sm text-hone-500 dark:text-hone-400 mb-4">
                    These transactions were skipped because they already exist in the database.
                  </p>
                  <div className="divide-y divide-hone-100 dark:divide-hone-800">
                    {skipped.map((tx) => (
                      <div
                        key={tx.id}
                        className="py-3 flex items-center justify-between hover:bg-hone-50 dark:hover:bg-hone-800/50 -mx-4 px-4"
                      >
                        <div className="flex-1 min-w-0">
                          <div className="text-sm font-medium text-hone-900 dark:text-hone-100 truncate">
                            {tx.description}
                          </div>
                          <div className="text-xs text-hone-500 dark:text-hone-400 mt-0.5">
                            {formatDate(tx.date)}
                            {tx.existing_transaction_id && (
                              <span className="ml-2 text-hone-400">
                                (matches tx #{tx.existing_transaction_id})
                              </span>
                            )}
                          </div>
                        </div>
                        <div className={`font-medium ${tx.amount < 0 ? "amount-negative" : "amount-positive"}`}>
                          ${Math.abs(tx.amount).toFixed(2)}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}

          {activeTab === "tagging" && (
            <div className="space-y-4">
              <p className="text-sm text-hone-500 dark:text-hone-400">
                Breakdown of how transactions were tagged during this import.
              </p>

              <div className="space-y-3">
                {s.tagged_by_learned > 0 && (
                  <div className="flex items-center justify-between p-3 bg-hone-50 dark:bg-hone-800 rounded-lg">
                    <div className="flex items-center gap-3">
                      <div className="p-2 bg-indigo-100 dark:bg-indigo-900/30 rounded-lg">
                        <CheckCircle className="w-4 h-4 text-indigo-600 dark:text-indigo-400" />
                      </div>
                      <div>
                        <div className="font-medium text-hone-900 dark:text-hone-100">By Learned</div>
                        <div className="text-xs text-hone-500 dark:text-hone-400">
                          From your previous manual tags
                        </div>
                      </div>
                    </div>
                    <div className="text-xl font-bold text-hone-900 dark:text-hone-100">
                      {s.tagged_by_learned}
                    </div>
                  </div>
                )}

                {s.tagged_by_rule > 0 && (
                  <div className="flex items-center justify-between p-3 bg-hone-50 dark:bg-hone-800 rounded-lg">
                    <div className="flex items-center gap-3">
                      <div className="p-2 bg-blue-100 dark:bg-blue-900/30 rounded-lg">
                        <FileText className="w-4 h-4 text-blue-600 dark:text-blue-400" />
                      </div>
                      <div>
                        <div className="font-medium text-hone-900 dark:text-hone-100">By Rules</div>
                        <div className="text-xs text-hone-500 dark:text-hone-400">
                          User-defined patterns matched
                        </div>
                      </div>
                    </div>
                    <div className="text-xl font-bold text-hone-900 dark:text-hone-100">
                      {s.tagged_by_rule}
                    </div>
                  </div>
                )}

                {s.tagged_by_pattern > 0 && (
                  <div className="flex items-center justify-between p-3 bg-hone-50 dark:bg-hone-800 rounded-lg">
                    <div className="flex items-center gap-3">
                      <div className="p-2 bg-green-100 dark:bg-green-900/30 rounded-lg">
                        <Tag className="w-4 h-4 text-green-600 dark:text-green-400" />
                      </div>
                      <div>
                        <div className="font-medium text-hone-900 dark:text-hone-100">By Patterns</div>
                        <div className="text-xs text-hone-500 dark:text-hone-400">
                          Auto-patterns on tags matched
                        </div>
                      </div>
                    </div>
                    <div className="text-xl font-bold text-hone-900 dark:text-hone-100">
                      {s.tagged_by_pattern}
                    </div>
                  </div>
                )}

                {s.tagged_by_bank_category > 0 && (
                  <div className="flex items-center justify-between p-3 bg-hone-50 dark:bg-hone-800 rounded-lg">
                    <div className="flex items-center gap-3">
                      <div className="p-2 bg-purple-100 dark:bg-purple-900/30 rounded-lg">
                        <CreditCard className="w-4 h-4 text-purple-600 dark:text-purple-400" />
                      </div>
                      <div>
                        <div className="font-medium text-hone-900 dark:text-hone-100">By Bank Category</div>
                        <div className="text-xs text-hone-500 dark:text-hone-400">
                          Bank-provided category mapped
                        </div>
                      </div>
                    </div>
                    <div className="text-xl font-bold text-hone-900 dark:text-hone-100">
                      {s.tagged_by_bank_category}
                    </div>
                  </div>
                )}

                {s.tagged_by_ollama > 0 && (
                  <div className="flex items-center justify-between p-3 bg-hone-50 dark:bg-hone-800 rounded-lg">
                    <div className="flex items-center gap-3">
                      <div className="p-2 bg-amber-100 dark:bg-amber-900/30 rounded-lg">
                        <span className="text-amber-600 dark:text-amber-400 font-bold text-sm">AI</span>
                      </div>
                      <div>
                        <div className="font-medium text-hone-900 dark:text-hone-100">By Ollama</div>
                        <div className="text-xs text-hone-500 dark:text-hone-400">
                          AI-classified merchants
                        </div>
                      </div>
                    </div>
                    <div className="text-xl font-bold text-hone-900 dark:text-hone-100">
                      {s.tagged_by_ollama}
                    </div>
                  </div>
                )}

                {s.tagged_fallback > 0 && (
                  <div className="flex items-center justify-between p-3 bg-hone-50 dark:bg-hone-800 rounded-lg">
                    <div className="flex items-center gap-3">
                      <div className="p-2 bg-gray-100 dark:bg-gray-900/30 rounded-lg">
                        <AlertTriangle className="w-4 h-4 text-gray-500 dark:text-gray-400" />
                      </div>
                      <div>
                        <div className="font-medium text-hone-900 dark:text-hone-100">Fallback (Other)</div>
                        <div className="text-xs text-hone-500 dark:text-hone-400">
                          No pattern matched, tagged as Other
                        </div>
                      </div>
                    </div>
                    <div className="text-xl font-bold text-hone-900 dark:text-hone-100">
                      {s.tagged_fallback}
                    </div>
                  </div>
                )}

                {totalTagged === 0 && (
                  <div className="text-center py-8 text-hone-500 dark:text-hone-400">
                    No tagging data available for this import
                  </div>
                )}
              </div>
            </div>
          )}

          {activeTab === "comparison" && (
            <div className="space-y-6">
              {comparisonLoading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 className="w-5 h-5 animate-spin text-hone-400" />
                </div>
              ) : comparison ? (
                <>
                  {/* Summary Changes */}
                  <div>
                    <h3 className="text-sm font-medium text-hone-700 dark:text-hone-300 mb-3 flex items-center gap-2">
                      <GitCompare className="w-4 h-4" />
                      Before / After Comparison
                    </h3>
                    <div className="grid grid-cols-2 gap-4">
                      {/* Tagging Changes */}
                      <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-4">
                        <h4 className="text-xs font-medium text-hone-500 dark:text-hone-400 mb-2">Tagging Breakdown</h4>
                        <table className="w-full text-sm">
                          <thead>
                            <tr className="text-hone-500 dark:text-hone-400 text-xs">
                              <th className="text-left pb-1">Source</th>
                              <th className="text-right pb-1">Before</th>
                              <th className="text-right pb-1">After</th>
                            </tr>
                          </thead>
                          <tbody className="text-hone-900 dark:text-hone-100">
                            <tr>
                              <td>Learned</td>
                              <td className="text-right">{comparison.before.tagging_breakdown.by_learned}</td>
                              <td className="text-right">{comparison.after.tagging_breakdown.by_learned}</td>
                            </tr>
                            <tr>
                              <td>Rules</td>
                              <td className="text-right">{comparison.before.tagging_breakdown.by_rule}</td>
                              <td className="text-right">{comparison.after.tagging_breakdown.by_rule}</td>
                            </tr>
                            <tr>
                              <td>Patterns</td>
                              <td className="text-right">{comparison.before.tagging_breakdown.by_pattern}</td>
                              <td className="text-right">{comparison.after.tagging_breakdown.by_pattern}</td>
                            </tr>
                            <tr>
                              <td>Ollama</td>
                              <td className="text-right">{comparison.before.tagging_breakdown.by_ollama}</td>
                              <td className="text-right">{comparison.after.tagging_breakdown.by_ollama}</td>
                            </tr>
                            <tr>
                              <td>Bank</td>
                              <td className="text-right">{comparison.before.tagging_breakdown.by_bank_category}</td>
                              <td className="text-right">{comparison.after.tagging_breakdown.by_bank_category}</td>
                            </tr>
                            <tr>
                              <td>Fallback</td>
                              <td className="text-right">{comparison.before.tagging_breakdown.fallback}</td>
                              <td className="text-right">{comparison.after.tagging_breakdown.fallback}</td>
                            </tr>
                          </tbody>
                        </table>
                      </div>

                      {/* Detection Changes */}
                      <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-4">
                        <h4 className="text-xs font-medium text-hone-500 dark:text-hone-400 mb-2">Detection Results</h4>
                        <table className="w-full text-sm">
                          <thead>
                            <tr className="text-hone-500 dark:text-hone-400 text-xs">
                              <th className="text-left pb-1">Type</th>
                              <th className="text-right pb-1">Before</th>
                              <th className="text-right pb-1">After</th>
                            </tr>
                          </thead>
                          <tbody className="text-hone-900 dark:text-hone-100">
                            <tr>
                              <td>Subscriptions</td>
                              <td className="text-right">{comparison.before.subscriptions_found}</td>
                              <td className="text-right">{comparison.after.subscriptions_found}</td>
                            </tr>
                            <tr>
                              <td>Zombies</td>
                              <td className="text-right">{comparison.before.zombies_detected}</td>
                              <td className="text-right">{comparison.after.zombies_detected}</td>
                            </tr>
                            <tr>
                              <td>Price Increases</td>
                              <td className="text-right">{comparison.before.price_increases_detected}</td>
                              <td className="text-right">{comparison.after.price_increases_detected}</td>
                            </tr>
                            <tr>
                              <td>Duplicates</td>
                              <td className="text-right">{comparison.before.duplicates_detected}</td>
                              <td className="text-right">{comparison.after.duplicates_detected}</td>
                            </tr>
                          </tbody>
                        </table>
                      </div>
                    </div>
                  </div>

                  {/* Tag Changes */}
                  {comparison.tag_changes.length > 0 && (
                    <div>
                      <h3 className="text-sm font-medium text-hone-700 dark:text-hone-300 mb-3 flex items-center gap-2">
                        <Tag className="w-4 h-4" />
                        Tag Changes ({comparison.tag_changes.length})
                      </h3>
                      <div className="divide-y divide-hone-100 dark:divide-hone-800">
                        {comparison.tag_changes.slice(0, 20).map((change) => (
                          <div key={change.transaction_id} className="py-2">
                            <div className="text-sm font-medium text-hone-900 dark:text-hone-100 truncate">
                              {change.description}
                            </div>
                            <div className="flex items-center gap-2 text-xs mt-1">
                              <span className="text-hone-500 dark:text-hone-400">
                                {change.before_tags.length > 0 ? change.before_tags.join(", ") : "(none)"}
                              </span>
                              <ArrowRight className="w-3 h-3 text-hone-400" />
                              <span className="text-savings">
                                {change.after_tags.length > 0 ? change.after_tags.join(", ") : "(none)"}
                              </span>
                            </div>
                          </div>
                        ))}
                        {comparison.tag_changes.length > 20 && (
                          <div className="py-2 text-sm text-hone-500 dark:text-hone-400">
                            +{comparison.tag_changes.length - 20} more changes
                          </div>
                        )}
                      </div>
                    </div>
                  )}

                  {/* Merchant Changes */}
                  {comparison.merchant_changes.length > 0 && (
                    <div>
                      <h3 className="text-sm font-medium text-hone-700 dark:text-hone-300 mb-3 flex items-center gap-2">
                        <FileText className="w-4 h-4" />
                        Merchant Name Changes ({comparison.merchant_changes.length})
                      </h3>
                      <div className="divide-y divide-hone-100 dark:divide-hone-800">
                        {comparison.merchant_changes.slice(0, 20).map((change) => (
                          <div key={change.transaction_id} className="py-2">
                            <div className="text-sm text-hone-500 dark:text-hone-400 truncate">
                              {change.description}
                            </div>
                            <div className="flex items-center gap-2 text-sm mt-1">
                              <span className="text-hone-600 dark:text-hone-300">
                                {change.before_merchant || "(none)"}
                              </span>
                              <ArrowRight className="w-3 h-3 text-hone-400" />
                              <span className="font-medium text-hone-900 dark:text-hone-100">
                                {change.after_merchant || "(none)"}
                              </span>
                            </div>
                          </div>
                        ))}
                        {comparison.merchant_changes.length > 20 && (
                          <div className="py-2 text-sm text-hone-500 dark:text-hone-400">
                            +{comparison.merchant_changes.length - 20} more changes
                          </div>
                        )}
                      </div>
                    </div>
                  )}

                  {comparison.tag_changes.length === 0 && comparison.merchant_changes.length === 0 && (
                    <div className="text-center py-8 text-hone-500 dark:text-hone-400">
                      <CheckCircle className="w-8 h-8 mx-auto mb-2 opacity-30" />
                      <p>No changes detected during reprocessing</p>
                    </div>
                  )}
                </>
              ) : (
                <div className="text-center py-8 text-hone-500 dark:text-hone-400">
                  No comparison data available. Reprocess to generate comparison.
                </div>
              )}
            </div>
          )}

          {activeTab === "history" && (
            <div className="space-y-6">
              {runsLoading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 className="w-5 h-5 animate-spin text-hone-400" />
                </div>
              ) : runs.length === 0 || (runs.length === 1 && s.status === "processing") ? (
                <div className="text-center py-8 text-hone-500 dark:text-hone-400">
                  {s.status === "processing" ? (
                    <>
                      <Loader2 className="w-8 h-8 mx-auto mb-2 animate-spin opacity-50" />
                      <p>Initial import in progress...</p>
                      <p className="text-sm mt-1">History will be available after processing completes</p>
                    </>
                  ) : (
                    <>
                      <History className="w-8 h-8 mx-auto mb-2 opacity-30" />
                      <p>No reprocess runs yet</p>
                      <p className="text-sm mt-1">Click "Reprocess" to generate your first run</p>
                    </>
                  )}
                </div>
              ) : (
                <>
                  {/* Run List */}
                  <div>
                    <h3 className="text-sm font-medium text-hone-700 dark:text-hone-300 mb-3">
                      History ({runs.length - 1} reprocess {runs.length - 1 === 1 ? "run" : "runs"})
                    </h3>
                    <div className="space-y-2">
                      {runs.map((run) => {
                        const isInitialImport = run.run_number === 0;
                        return (
                          <div
                            key={run.id}
                            className={`p-3 rounded-lg border transition-colors ${
                              selectedRunA === run.id || selectedRunB === run.id
                                ? "border-hone-500 bg-hone-50 dark:bg-hone-800"
                                : "border-hone-200 dark:border-hone-700 hover:bg-hone-50 dark:hover:bg-hone-800/50"
                            }`}
                          >
                            <div className="flex items-center justify-between">
                              <div className="flex items-center gap-3">
                                <div className="flex items-center gap-2">
                                  <span className="text-sm font-medium text-hone-900 dark:text-hone-100">
                                    {isInitialImport ? "Initial Import" : `Run #${run.run_number}`}
                                  </span>
                                  {run.status === "running" && (
                                    <Loader2 className="w-3.5 h-3.5 animate-spin text-hone-400" />
                                  )}
                                  {run.status === "completed" && (
                                    <CheckCircle className="w-3.5 h-3.5 text-savings" />
                                  )}
                                  {run.status === "failed" && (
                                    <AlertTriangle className="w-3.5 h-3.5 text-waste" />
                                  )}
                                </div>
                                {run.ollama_model && (
                                  <span className="text-xs px-1.5 py-0.5 rounded bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-400">
                                    {run.ollama_model}
                                  </span>
                                )}
                              </div>
                              <div className="flex items-center gap-2">
                                {run.status === "completed" && (
                                  <>
                                    <button
                                      onClick={() => setSelectedRunA(selectedRunA === run.id ? null : run.id)}
                                      className={`text-xs px-2 py-1 rounded transition-colors ${
                                        selectedRunA === run.id
                                          ? "bg-blue-500 text-white"
                                          : "bg-hone-200 dark:bg-hone-700 text-hone-600 dark:text-hone-300 hover:bg-blue-100 dark:hover:bg-blue-900/30"
                                      }`}
                                    >
                                      A
                                    </button>
                                    <button
                                      onClick={() => setSelectedRunB(selectedRunB === run.id ? null : run.id)}
                                      className={`text-xs px-2 py-1 rounded transition-colors ${
                                        selectedRunB === run.id
                                          ? "bg-green-500 text-white"
                                          : "bg-hone-200 dark:bg-hone-700 text-hone-600 dark:text-hone-300 hover:bg-green-100 dark:hover:bg-green-900/30"
                                      }`}
                                    >
                                      B
                                    </button>
                                  </>
                                )}
                              </div>
                            </div>
                            <div className="flex items-center gap-4 mt-2 text-xs text-hone-500 dark:text-hone-400">
                              <span>{formatDateTime(run.started_at)}</span>
                              {run.initiated_by && <span>by {run.initiated_by}</span>}
                              {run.status === "running" && s.processing_phase && (
                                <span className="text-hone-600 dark:text-hone-300">
                                  {getPhaseDescription(s.processing_phase)}
                                  {s.processing_total > 0 && ` (${s.processing_current}/${s.processing_total})`}
                                </span>
                              )}
                              {run.status === "completed" && !isInitialImport && (
                                <span className="text-hone-600 dark:text-hone-300">
                                  {run.tags_changed} tag changes, {run.merchants_changed} merchant changes
                                </span>
                              )}
                              {isInitialImport && (
                                <span className="text-hone-600 dark:text-hone-300">
                                  Baseline for comparison
                                </span>
                              )}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </div>

                  {/* Run Comparison */}
                  {selectedRunA !== null && selectedRunB !== null && selectedRunA !== selectedRunB && (
                    <div>
                      <h3 className="text-sm font-medium text-hone-700 dark:text-hone-300 mb-3 flex items-center gap-2">
                        <GitCompare className="w-4 h-4" />
                        Comparing{" "}
                        <span className="px-1.5 py-0.5 rounded bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-400">
                          {(() => {
                            const runA = runs.find((r) => r.id === selectedRunA);
                            if (!runA) return "?";
                            if (runA.run_number === 0) return runA.ollama_model || "Initial Import";
                            return runA.ollama_model || `Run #${runA.run_number}`;
                          })()}
                        </span>
                        <ArrowRight className="w-3 h-3" />
                        <span className="px-1.5 py-0.5 rounded bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400">
                          {(() => {
                            const runB = runs.find((r) => r.id === selectedRunB);
                            if (!runB) return "?";
                            if (runB.run_number === 0) return runB.ollama_model || "Initial Import";
                            return runB.ollama_model || `Run #${runB.run_number}`;
                          })()}
                        </span>
                      </h3>
                      {runComparisonLoading ? (
                        <div className="flex items-center justify-center py-8">
                          <Loader2 className="w-5 h-5 animate-spin text-hone-400" />
                        </div>
                      ) : runComparison ? (
                        <div className="space-y-4">
                          {/* Tagging Diff */}
                          <div className="grid grid-cols-2 gap-4">
                            <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-4">
                              <h4 className="text-xs font-medium text-hone-500 dark:text-hone-400 mb-2">Tagging Changes</h4>
                              <div className="space-y-1 text-sm">
                                {runComparison.tagging_diff.learned_diff !== 0 && (
                                  <div className="flex justify-between">
                                    <span>Learned</span>
                                    <span className={runComparison.tagging_diff.learned_diff > 0 ? "text-savings" : "text-waste"}>
                                      {runComparison.tagging_diff.learned_diff > 0 ? "+" : ""}{runComparison.tagging_diff.learned_diff}
                                    </span>
                                  </div>
                                )}
                                {runComparison.tagging_diff.rule_diff !== 0 && (
                                  <div className="flex justify-between">
                                    <span>Rules</span>
                                    <span className={runComparison.tagging_diff.rule_diff > 0 ? "text-savings" : "text-waste"}>
                                      {runComparison.tagging_diff.rule_diff > 0 ? "+" : ""}{runComparison.tagging_diff.rule_diff}
                                    </span>
                                  </div>
                                )}
                                {runComparison.tagging_diff.pattern_diff !== 0 && (
                                  <div className="flex justify-between">
                                    <span>Patterns</span>
                                    <span className={runComparison.tagging_diff.pattern_diff > 0 ? "text-savings" : "text-waste"}>
                                      {runComparison.tagging_diff.pattern_diff > 0 ? "+" : ""}{runComparison.tagging_diff.pattern_diff}
                                    </span>
                                  </div>
                                )}
                                {runComparison.tagging_diff.ollama_diff !== 0 && (
                                  <div className="flex justify-between">
                                    <span>Ollama</span>
                                    <span className={runComparison.tagging_diff.ollama_diff > 0 ? "text-savings" : "text-waste"}>
                                      {runComparison.tagging_diff.ollama_diff > 0 ? "+" : ""}{runComparison.tagging_diff.ollama_diff}
                                    </span>
                                  </div>
                                )}
                                {runComparison.tagging_diff.fallback_diff !== 0 && (
                                  <div className="flex justify-between">
                                    <span>Fallback</span>
                                    <span className={runComparison.tagging_diff.fallback_diff < 0 ? "text-savings" : "text-waste"}>
                                      {runComparison.tagging_diff.fallback_diff > 0 ? "+" : ""}{runComparison.tagging_diff.fallback_diff}
                                    </span>
                                  </div>
                                )}
                                {Object.values(runComparison.tagging_diff).every((v) => v === 0) && (
                                  <span className="text-hone-400">No changes</span>
                                )}
                              </div>
                            </div>
                            <div className="bg-hone-50 dark:bg-hone-800 rounded-lg p-4">
                              <h4 className="text-xs font-medium text-hone-500 dark:text-hone-400 mb-2">Detection Changes</h4>
                              <div className="space-y-1 text-sm">
                                {runComparison.detection_diff.subscriptions_diff !== 0 && (
                                  <div className="flex justify-between">
                                    <span>Subscriptions</span>
                                    <span className={runComparison.detection_diff.subscriptions_diff > 0 ? "text-savings" : "text-waste"}>
                                      {runComparison.detection_diff.subscriptions_diff > 0 ? "+" : ""}{runComparison.detection_diff.subscriptions_diff}
                                    </span>
                                  </div>
                                )}
                                {runComparison.detection_diff.zombies_diff !== 0 && (
                                  <div className="flex justify-between">
                                    <span>Zombies</span>
                                    <span>{runComparison.detection_diff.zombies_diff > 0 ? "+" : ""}{runComparison.detection_diff.zombies_diff}</span>
                                  </div>
                                )}
                                {Object.values(runComparison.detection_diff).every((v) => v === 0) && (
                                  <span className="text-hone-400">No changes</span>
                                )}
                              </div>
                            </div>
                          </div>

                          {/* Tag Differences */}
                          {runComparison.tag_differences.length > 0 && (
                            <div>
                              <h4 className="text-xs font-medium text-hone-500 dark:text-hone-400 mb-2">
                                Tag Differences ({runComparison.tag_differences.length})
                              </h4>
                              <div className="divide-y divide-hone-100 dark:divide-hone-800 max-h-40 overflow-y-auto">
                                {runComparison.tag_differences.slice(0, 10).map((diff) => (
                                  <div key={diff.transaction_id} className="py-2">
                                    <div className="text-sm text-hone-900 dark:text-hone-100 truncate">{diff.description}</div>
                                    <div className="flex items-center gap-2 text-xs mt-1">
                                      <span className="text-hone-500">{diff.run_a_tags.join(", ") || "(none)"}</span>
                                      <ArrowRight className="w-3 h-3 text-hone-400" />
                                      <span className="text-savings">{diff.run_b_tags.join(", ") || "(none)"}</span>
                                    </div>
                                  </div>
                                ))}
                                {runComparison.tag_differences.length > 10 && (
                                  <div className="py-2 text-xs text-hone-500">
                                    +{runComparison.tag_differences.length - 10} more
                                  </div>
                                )}
                              </div>
                            </div>
                          )}

                          {/* Merchant Differences */}
                          {runComparison.merchant_differences.length > 0 && (
                            <div>
                              <h4 className="text-xs font-medium text-hone-500 dark:text-hone-400 mb-2">
                                Merchant Differences ({runComparison.merchant_differences.length})
                              </h4>
                              <div className="divide-y divide-hone-100 dark:divide-hone-800 max-h-40 overflow-y-auto">
                                {runComparison.merchant_differences.slice(0, 10).map((diff) => (
                                  <div key={diff.transaction_id} className="py-2">
                                    <div className="text-xs text-hone-500 truncate">{diff.description}</div>
                                    <div className="flex items-center gap-2 text-sm mt-1">
                                      <span className="text-hone-600 dark:text-hone-300">{diff.run_a_merchant || "(none)"}</span>
                                      <ArrowRight className="w-3 h-3 text-hone-400" />
                                      <span className="font-medium text-hone-900 dark:text-hone-100">{diff.run_b_merchant || "(none)"}</span>
                                    </div>
                                  </div>
                                ))}
                                {runComparison.merchant_differences.length > 10 && (
                                  <div className="py-2 text-xs text-hone-500">
                                    +{runComparison.merchant_differences.length - 10} more
                                  </div>
                                )}
                              </div>
                            </div>
                          )}

                          {runComparison.tag_differences.length === 0 && runComparison.merchant_differences.length === 0 && (
                            <div className="text-center py-4 text-hone-500 dark:text-hone-400 text-sm">
                              No transaction-level differences between these runs
                            </div>
                          )}
                        </div>
                      ) : (
                        <div className="text-center py-4 text-hone-500 dark:text-hone-400 text-sm">
                          Select two completed runs to compare
                        </div>
                      )}
                    </div>
                  )}

                  {runs.length === 1 && (
                    <div className="text-center py-4 text-hone-500 dark:text-hone-400 text-sm">
                      Reprocess again to compare different model results
                    </div>
                  )}
                </>
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-4 py-3 border-t border-hone-200 dark:border-hone-700 flex justify-between items-center">
          <div>
            {comparison && s.status === "completed" && (
              <div className="text-sm text-hone-500 dark:text-hone-400 flex items-center gap-1">
                <GitCompare className="w-4 h-4" />
                {comparison.tag_changes.length + comparison.merchant_changes.length} changes
              </div>
            )}
          </div>
          <div className="flex items-center gap-3">
            {/* Model selector for reprocess (hidden during initial processing) */}
            {availableModels.length > 0 && !(s.status === "processing" && runs.length <= 1 && !reprocessing) && (
              <div className="relative">
                <button
                  onClick={() => setShowModelSelector(!showModelSelector)}
                  className="flex items-center gap-2 px-3 py-2 text-sm
                            bg-hone-100 dark:bg-hone-800 rounded-lg
                            hover:bg-hone-200 dark:hover:bg-hone-700 transition-colors
                            text-hone-700 dark:text-hone-300"
                  disabled={reprocessing || s.status === "processing"}
                >
                  <Settings className="w-4 h-4" />
                  <span className="max-w-[100px] truncate">
                    {loadingModels ? "..." : selectedModel || "Model"}
                  </span>
                  <ChevronDown className="w-3 h-3" />
                </button>

                {showModelSelector && (
                  <div className="absolute right-0 bottom-full mb-1 w-56 bg-white dark:bg-hone-800
                                border border-hone-200 dark:border-hone-700 rounded-lg shadow-lg z-10
                                max-h-60 overflow-y-auto">
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
                          <span className="text-xs text-hone-400 ml-1">(default)</span>
                        )}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}

            {/* Hide reprocess button during initial processing (no reprocess runs yet) */}
            {!(s.status === "processing" && runs.length <= 1 && !reprocessing) && (
              <button
                onClick={handleReprocess}
                disabled={reprocessing || s.status === "processing" || s.imported_count === 0}
                className="btn-secondary flex items-center gap-2"
              >
                {reprocessing || (s.status === "processing" && runs.length > 1) ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Reprocessing...
                  </>
                ) : (
                  <>
                    <RefreshCw className="w-4 h-4" />
                    Reprocess
                  </>
                )}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
