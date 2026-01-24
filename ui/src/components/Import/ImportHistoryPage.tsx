import { AlertCircle, Ban, Calendar, Check, ChevronLeft, ChevronRight, CreditCard, FileText, Loader2, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../api";
import { useHashRouter } from "../../hooks";
import type { Account, ImportSessionWithAccount } from "../../types";
import { ImportDetailModal } from "./ImportDetailModal";

interface ImportHistoryPageProps {
  accounts: Account[];
}

export function ImportHistoryPage({ accounts }: ImportHistoryPageProps) {
  const { state: routerState, setSubview } = useHashRouter();
  const [sessions, setSessions] = useState<ImportSessionWithAccount[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [accountFilter, setAccountFilter] = useState<number | undefined>(undefined);
  const [page, setPage] = useState(0);
  const pageSize = 20;
  const pollIntervalRef = useRef<number | null>(null);

  // Get selected session ID from URL subview
  const selectedSessionId = routerState.subview ? parseInt(routerState.subview, 10) : null;

  // Find the selected session from loaded sessions, or fetch it
  const [selectedSession, setSelectedSession] = useState<ImportSessionWithAccount | null>(null);

  const loadSessions = useCallback(async (showLoadingState = true) => {
    try {
      if (showLoadingState) setLoading(true);
      const response = await api.getImportSessions({
        account_id: accountFilter,
        limit: pageSize,
        offset: page * pageSize,
      });
      setSessions(response.sessions);
      setTotal(response.total);
    } catch (err) {
      console.error("Failed to load import sessions:", err);
    } finally {
      if (showLoadingState) setLoading(false);
    }
  }, [accountFilter, page]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  // Poll for updates when there are processing sessions or selected session is processing
  useEffect(() => {
    const hasProcessingInList = sessions.some(
      (s) => s.session.status === "pending" || s.session.status === "processing"
    );
    const selectedIsProcessing = selectedSession &&
      (selectedSession.session.status === "pending" || selectedSession.session.status === "processing");

    if (hasProcessingInList || selectedIsProcessing) {
      // Poll every 2 seconds
      pollIntervalRef.current = window.setInterval(() => {
        loadSessions(false); // Don't show loading state for polls
        // Also refresh the selected session if it exists
        if (selectedSessionId) {
          api.getImportSession(selectedSessionId)
            .then(setSelectedSession)
            .catch(() => {}); // Ignore errors during poll
        }
      }, 2000);
    }

    return () => {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
    };
  }, [sessions, selectedSession, selectedSessionId, loadSessions]);

  // Load selected session from URL when sessions change or on mount
  useEffect(() => {
    if (selectedSessionId) {
      // First check if it's already in loaded sessions
      const found = sessions.find((s) => s.session.id === selectedSessionId);
      if (found) {
        setSelectedSession(found);
      } else {
        // Fetch it directly - always fetch if not found in list
        // This handles both the initial load and when session is on a different page
        api.getImportSession(selectedSessionId)
          .then(setSelectedSession)
          .catch(() => setSubview(null)); // Clear invalid ID from URL
      }
    } else {
      setSelectedSession(null);
    }
  }, [selectedSessionId, sessions, setSubview]);

  const handleSelectSession = (session: ImportSessionWithAccount) => {
    setSubview(String(session.session.id));
  };

  const handleCloseDetail = () => {
    setSubview(null);
  };

  const formatDate = (dateStr: string) => {
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

  const totalPages = Math.ceil(total / pageSize);

  const getTaggingTotal = (session: ImportSessionWithAccount) => {
    const s = session.session;
    return s.tagged_by_learned + s.tagged_by_rule + s.tagged_by_pattern + s.tagged_by_ollama + s.tagged_by_bank_category + s.tagged_fallback;
  };

  // Map technical phase names to user-friendly descriptions
  const getPhaseDescription = (phase: string | null): string => {
    if (!phase) return "Starting...";
    switch (phase) {
      case "tagging":
        return "Categorizing";
      case "normalizing":
        return "Extracting merchants";
      case "matching_receipts":
        return "Matching receipts";
      case "detecting":
        return "Detecting alerts";
      case "classifying_merchants":
        return "Classifying merchants";
      case "analyzing_duplicates":
        return "Analyzing duplicates";
      case "analyzing_spending":
        return "Analyzing spending";
      default:
        return phase;
    }
  };

  const getStatusBadge = (session: ImportSessionWithAccount) => {
    const s = session.session;
    switch (s.status) {
      case "pending":
      case "processing":
        return (
          <span className="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-hone-100 dark:bg-hone-800 text-hone-600 dark:text-hone-400">
            <RefreshCw className="w-3 h-3 animate-spin" />
            {getPhaseDescription(s.processing_phase)}
            {s.processing_total > 0 && ` (${s.processing_current}/${s.processing_total})`}
          </span>
        );
      case "failed":
        return (
          <span className="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-waste/10 text-waste">
            <AlertCircle className="w-3 h-3" />
            Failed
          </span>
        );
      case "cancelled":
        return (
          <span className="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-attention/10 text-attention">
            <Ban className="w-3 h-3" />
            Cancelled
          </span>
        );
      case "completed":
      default:
        return (
          <span className="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-savings/10 text-savings">
            <Check className="w-3 h-3" />
            Complete
          </span>
        );
    }
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-hone-900 dark:text-hone-100">Import History</h1>
          <p className="text-hone-500 dark:text-hone-400 mt-1">
            Browse past imports and view details
          </p>
        </div>

        {/* Account Filter */}
        <div className="flex items-center gap-2">
          <label className="text-sm text-hone-600 dark:text-hone-400">Account:</label>
          <select
            value={accountFilter ?? ""}
            onChange={(e) => {
              setAccountFilter(e.target.value ? Number(e.target.value) : undefined);
              setPage(0);
            }}
            className="input-field py-1.5 text-sm min-w-[150px]"
          >
            <option value="">All accounts</option>
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Sessions Table */}
      <div className="card overflow-hidden">
        {loading ? (
          <div className="flex items-center justify-center py-12">
            <Loader2 className="w-6 h-6 animate-spin text-hone-400" />
          </div>
        ) : sessions.length === 0 ? (
          <div className="text-center py-12 text-hone-500 dark:text-hone-400">
            <FileText className="w-12 h-12 mx-auto mb-3 opacity-30" />
            <p>No imports found</p>
            {accountFilter && (
              <p className="text-sm mt-1">Try selecting a different account filter</p>
            )}
          </div>
        ) : (
          <>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="bg-hone-50 dark:bg-hone-800 border-b border-hone-200 dark:border-hone-700">
                    <th className="text-left px-4 py-3 font-medium text-hone-600 dark:text-hone-400">
                      Date
                    </th>
                    <th className="text-left px-4 py-3 font-medium text-hone-600 dark:text-hone-400">
                      Account
                    </th>
                    <th className="text-left px-4 py-3 font-medium text-hone-600 dark:text-hone-400">
                      Status
                    </th>
                    <th className="text-right px-4 py-3 font-medium text-hone-600 dark:text-hone-400">
                      Imported
                    </th>
                    <th className="text-right px-4 py-3 font-medium text-hone-600 dark:text-hone-400 hidden sm:table-cell">
                      Tagged
                    </th>
                    <th className="text-right px-4 py-3 font-medium text-hone-600 dark:text-hone-400 hidden md:table-cell">
                      Size
                    </th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-hone-100 dark:divide-hone-800">
                  {sessions.map((session) => (
                    <tr
                      key={session.session.id}
                      className="hover:bg-hone-50 dark:hover:bg-hone-800/50 cursor-pointer transition-colors"
                      onClick={() => handleSelectSession(session)}
                    >
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2 text-hone-900 dark:text-hone-100">
                          <Calendar className="w-4 h-4 text-hone-400" />
                          {formatDate(session.session.created_at)}
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2">
                          <CreditCard className="w-4 h-4 text-hone-400" />
                          <span className="text-hone-900 dark:text-hone-100">
                            {session.account_name}
                          </span>
                          <span className="text-xs text-hone-400 uppercase">
                            {session.session.bank}
                          </span>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        {getStatusBadge(session)}
                      </td>
                      <td className="px-4 py-3 text-right">
                        <span className="font-medium text-savings">
                          {session.session.imported_count}
                        </span>
                        {session.session.skipped_count > 0 && (
                          <span className="text-xs text-attention ml-1">
                            (+{session.session.skipped_count} dup)
                          </span>
                        )}
                      </td>
                      <td className="px-4 py-3 text-right hidden sm:table-cell">
                        <span className="text-hone-700 dark:text-hone-300">
                          {getTaggingTotal(session)}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-right hidden md:table-cell">
                        <span className="text-hone-500 dark:text-hone-400">
                          {formatFileSize(session.session.file_size_bytes)}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {/* Pagination */}
            {totalPages > 1 && (
              <div className="flex items-center justify-between px-4 py-3 border-t border-hone-200 dark:border-hone-700">
                <div className="text-sm text-hone-500 dark:text-hone-400">
                  Showing {page * pageSize + 1}-{Math.min((page + 1) * pageSize, total)} of {total}
                </div>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => setPage((p) => Math.max(0, p - 1))}
                    disabled={page === 0}
                    className="p-1.5 rounded hover:bg-hone-100 dark:hover:bg-hone-800 disabled:opacity-30 disabled:cursor-not-allowed"
                  >
                    <ChevronLeft className="w-5 h-5" />
                  </button>
                  <span className="text-sm text-hone-600 dark:text-hone-400">
                    Page {page + 1} of {totalPages}
                  </span>
                  <button
                    onClick={() => setPage((p) => Math.min(totalPages - 1, p + 1))}
                    disabled={page >= totalPages - 1}
                    className="p-1.5 rounded hover:bg-hone-100 dark:hover:bg-hone-800 disabled:opacity-30 disabled:cursor-not-allowed"
                  >
                    <ChevronRight className="w-5 h-5" />
                  </button>
                </div>
              </div>
            )}
          </>
        )}
      </div>

      {/* Detail Modal */}
      {selectedSession && (
        <ImportDetailModal
          session={selectedSession}
          onClose={handleCloseDetail}
        />
      )}
    </div>
  );
}
