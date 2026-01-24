// Type-safe API client for Hone backend

import type {
  Account,
  Alert,
  Bank,
  BulkTagsResponse,
  CancelImportResponse,
  DashboardStats,
  DetectionResults,
  Entity,
  EntityType,
  ExploreModelsResponse,
  ExploreResponse,
  FeedbackContext,
  FeedbackResponse,
  FeedbackStats,
  FeedbackTargetType,
  FeedbackType,
  Granularity,
  ImportResponse,
  ImportSessionsResponse,
  ImportSessionWithAccount,
  ImportTransactionsResponse,
  InsightFinding,
  InsightRefreshResponse,
  InsightStatus,
  InsightType,
  Location,
  LocationType,
  MerchantsReport,
  ModelComparisonStats,
  ModelRecommendation,
  OllamaHealthStatus,
  OllamaMetric,
  OllamaStats,
  PatternType,
  PendingReceiptResponse,
  Receipt,
  ReceiptParseResponse,
  ReceiptStatus,
  ReceiptUploadResponse,
  ReprocessComparison,
  ReprocessRunSummary,
  ReprocessRunWithComparison,
  ReprocessStartResponse,
  RunComparison,
  SavingsReport,
  SkippedTransaction,
  SplitType,
  SpendingSummary,
  Subscription,
  SubscriptionSummaryReport,
  Tag,
  TagRule,
  TagWithPath,
  TransactionResponse,
  TransactionSplit,
  TransactionSplitWithDetails,
  TransactionTag,
  TrendsReport,
  UserFeedback,
} from "./types";

const API_BASE = "/api";

class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
    this.name = "ApiError";
  }
}

async function fetchJson<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(`${API_BASE}${url}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options?.headers,
    },
  });

  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: "Unknown error" }));
    throw new ApiError(response.status, error.error || "Request failed");
  }

  return response.json();
}

export interface MeResponse {
  user: string;
  auth_method: string;
}

export const api = {
  // Auth
  getMe: () => fetchJson<MeResponse>("/me"),

  // Dashboard
  getDashboard: () => fetchJson<DashboardStats>("/dashboard"),

  // Accounts
  getAccounts: () => fetchJson<Account[]>("/accounts"),
  getAccount: (id: number) => fetchJson<Account>(`/accounts/${id}`),
  updateAccountEntity: (accountId: number, entityId: number | null) =>
    fetchJson<{ success: boolean }>(`/accounts/${accountId}/entity`, {
      method: "PATCH",
      body: JSON.stringify({ entity_id: entityId }),
    }),
  updateAccount: (id: number, data: { name: string; bank: Bank }) =>
    fetchJson<Account>(`/accounts/${id}`, {
      method: "PUT",
      body: JSON.stringify(data),
    }),
  deleteAccount: (id: number) =>
    fetchJson<{ success: boolean }>(`/accounts/${id}`, {
      method: "DELETE",
    }),
  createAccount: (name: string, bank: Bank) =>
    fetchJson<Account>("/accounts", {
      method: "POST",
      body: JSON.stringify({ name, bank }),
    }),

  // Transactions
  getTransactions: (params?: {
    limit?: number;
    offset?: number;
    account_id?: number;
    entity_id?: number;
    card_member?: string;
    search?: string;
    tag_ids?: number[];
    untagged?: boolean;
    period?: string;
    from?: string;
    to?: string;
    sort?: string;
    order?: string;
  }) => {
    const searchParams = new URLSearchParams();
    if (params?.limit) searchParams.set("limit", params.limit.toString());
    if (params?.offset) searchParams.set("offset", params.offset.toString());
    if (params?.account_id) searchParams.set("account_id", params.account_id.toString());
    if (params?.entity_id) searchParams.set("entity_id", params.entity_id.toString());
    if (params?.card_member) searchParams.set("card_member", params.card_member);
    if (params?.search) searchParams.set("search", params.search);
    if (params?.tag_ids && params.tag_ids.length > 0) {
      searchParams.set("tag_ids", params.tag_ids.join(","));
    }
    if (params?.untagged) searchParams.set("untagged", "true");
    if (params?.period) searchParams.set("period", params.period);
    if (params?.from) searchParams.set("from", params.from);
    if (params?.to) searchParams.set("to", params.to);
    if (params?.sort) searchParams.set("sort", params.sort);
    if (params?.order) searchParams.set("order", params.order);

    const query = searchParams.toString();
    return fetchJson<TransactionResponse>(`/transactions${query ? `?${query}` : ""}`);
  },

  updateTransactionLocation: (
    transactionId: number,
    data: { purchase_location_id?: number | null; vendor_location_id?: number | null }
  ) =>
    fetchJson<{ success: boolean }>(`/transactions/${transactionId}/location`, {
      method: "POST",
      body: JSON.stringify(data),
    }),

  archiveTransaction: (transactionId: number) =>
    fetchJson<{ success: boolean }>(`/transactions/${transactionId}/archive`, {
      method: "POST",
    }),

  updateMerchantName: (transactionId: number, merchantName: string) =>
    fetchJson<{ success: boolean; updated_count: number }>(
      `/transactions/${transactionId}/merchant`,
      {
        method: "PUT",
        body: JSON.stringify({ merchant_name: merchantName }),
      }
    ),

  // Subscriptions
  getSubscriptions: (params?: { account_id?: number }) => {
    const searchParams = new URLSearchParams();
    if (params?.account_id) searchParams.set("account_id", String(params.account_id));
    const query = searchParams.toString();
    return fetchJson<Subscription[]>(`/subscriptions${query ? `?${query}` : ""}`);
  },

  acknowledgeSubscription: (id: number) =>
    fetchJson<{ success: boolean }>(`/subscriptions/${id}/acknowledge`, {
      method: "POST",
    }),

  cancelSubscription: (id: number) =>
    fetchJson<{ success: boolean }>(`/subscriptions/${id}/cancel`, {
      method: "POST",
    }),

  excludeSubscription: (id: number) =>
    fetchJson<{ success: boolean }>(`/subscriptions/${id}/exclude`, {
      method: "POST",
    }),

  unexcludeSubscription: (id: number) =>
    fetchJson<{ success: boolean }>(`/subscriptions/${id}/unexclude`, {
      method: "POST",
    }),

  deleteSubscription: (id: number) =>
    fetchJson<{ success: boolean }>(`/subscriptions/${id}`, {
      method: "DELETE",
    }),

  // Alerts
  getAlerts: (includeDismissed = false) => fetchJson<Alert[]>(`/alerts?include_dismissed=${includeDismissed}`),

  dismissAlert: (id: number) =>
    fetchJson<{ success: boolean }>(`/alerts/${id}/dismiss`, {
      method: "POST",
    }),

  dismissAlertExclude: (id: number) =>
    fetchJson<{ success: boolean }>(`/alerts/${id}/dismiss-exclude`, {
      method: "POST",
    }),

  restoreAlert: (id: number) =>
    fetchJson<{ success: boolean }>(`/alerts/${id}/restore`, {
      method: "POST",
    }),

  reanalyzeSpendingAlert: (id: number) =>
    fetchJson<Alert>(`/alerts/${id}/reanalyze`, {
      method: "POST",
    }),

  // Detection
  runDetection: (kind: "all" | "zombies" | "increases" | "duplicates" = "all") =>
    fetchJson<DetectionResults>("/detect", {
      method: "POST",
      body: JSON.stringify({ kind }),
    }),

  // Insights
  getTopInsights: (limit = 5) =>
    fetchJson<InsightFinding[]>(`/insights?limit=${limit}`),

  listInsights: (params?: { status?: InsightStatus; insight_type?: InsightType }) => {
    const searchParams = new URLSearchParams();
    if (params?.status) searchParams.set("status", params.status);
    if (params?.insight_type) searchParams.set("insight_type", params.insight_type);
    const query = searchParams.toString();
    return fetchJson<InsightFinding[]>(`/insights/all${query ? `?${query}` : ""}`);
  },

  getInsight: (id: number) =>
    fetchJson<InsightFinding>(`/insights/${id}`),

  dismissInsight: (id: number) =>
    fetchJson<{ success: boolean }>(`/insights/${id}/dismiss`, {
      method: "POST",
    }),

  snoozeInsight: (id: number, days: number) =>
    fetchJson<{ success: boolean }>(`/insights/${id}/snooze`, {
      method: "POST",
      body: JSON.stringify({ days }),
    }),

  restoreInsight: (id: number) =>
    fetchJson<{ success: boolean }>(`/insights/${id}/restore`, {
      method: "POST",
    }),

  setInsightFeedback: (id: number, feedback: string) =>
    fetchJson<{ success: boolean }>(`/insights/${id}/feedback`, {
      method: "POST",
      body: JSON.stringify({ feedback }),
    }),

  refreshInsights: () =>
    fetchJson<InsightRefreshResponse>("/insights/refresh", {
      method: "POST",
    }),

  countInsights: () =>
    fetchJson<number>("/insights/count"),

  // Import CSV
  importCsv: async (file: File, accountId: number, model?: string): Promise<ImportResponse> => {
    const formData = new FormData();
    formData.append("file", file);
    formData.append("account_id", accountId.toString());
    if (model) {
      formData.append("model", model);
    }

    const response = await fetch(`${API_BASE}/import`, {
      method: "POST",
      body: formData,
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({ error: "Unknown error" }));
      throw new ApiError(response.status, error.error || "Import failed");
    }

    return response.json();
  },

  // Import History
  getImportSessions: (params?: { account_id?: number; limit?: number; offset?: number }) => {
    const searchParams = new URLSearchParams();
    if (params?.account_id) searchParams.set("account_id", params.account_id.toString());
    if (params?.limit) searchParams.set("limit", params.limit.toString());
    if (params?.offset) searchParams.set("offset", params.offset.toString());
    const query = searchParams.toString();
    return fetchJson<ImportSessionsResponse>(`/imports${query ? `?${query}` : ""}`);
  },

  getImportSession: (id: number) => fetchJson<ImportSessionWithAccount>(`/imports/${id}`),

  getImportSessionTransactions: (id: number, params?: { limit?: number; offset?: number }) => {
    const searchParams = new URLSearchParams();
    if (params?.limit) searchParams.set("limit", params.limit.toString());
    if (params?.offset) searchParams.set("offset", params.offset.toString());
    const query = searchParams.toString();
    return fetchJson<ImportTransactionsResponse>(
      `/imports/${id}/transactions${query ? `?${query}` : ""}`
    );
  },

  getImportSessionSkipped: (id: number) => fetchJson<SkippedTransaction[]>(`/imports/${id}/skipped`),

  cancelImportSession: (id: number) =>
    fetchJson<CancelImportResponse>(`/imports/${id}/cancel`, { method: "POST" }),

  reprocessImportSession: (id: number, model?: string) =>
    fetchJson<ReprocessStartResponse>(`/imports/${id}/reprocess`, {
      method: "POST",
      body: model ? JSON.stringify({ model }) : undefined,
    }),

  getReprocessComparison: (id: number) =>
    fetchJson<ReprocessComparison | null>(`/imports/${id}/reprocess-comparison`),

  // Reprocess Runs (historical comparison)
  getReprocessRuns: (sessionId: number) =>
    fetchJson<ReprocessRunSummary[]>(`/imports/${sessionId}/runs`),

  getReprocessRun: (sessionId: number, runId: number) =>
    fetchJson<ReprocessRunWithComparison>(`/imports/${sessionId}/runs/${runId}`),

  compareReprocessRuns: (sessionId: number, runAId: number, runBId: number) =>
    fetchJson<RunComparison>(`/imports/${sessionId}/runs/compare?run_a=${runAId}&run_b=${runBId}`),

  // Tags
  getTags: () => fetchJson<Tag[]>("/tags"),

  getTagsTree: () => fetchJson<TagWithPath[]>("/tags/tree"),

  getTransactionTags: (transactionId: number) =>
    fetchJson<TransactionTag[]>(`/transactions/${transactionId}/tags`),

  addTransactionTag: (transactionId: number, tagId: number) =>
    fetchJson<{ success: boolean }>(`/transactions/${transactionId}/tags`, {
      method: "POST",
      body: JSON.stringify({ tag_id: tagId }),
    }),

  removeTransactionTag: (transactionId: number, tagId: number) =>
    fetchJson<{ success: boolean }>(`/transactions/${transactionId}/tags/${tagId}`, {
      method: "DELETE",
    }),

  bulkAddTags: (transactionIds: number[], tagIds: number[]) =>
    fetchJson<BulkTagsResponse>("/transactions/bulk-tags", {
      method: "POST",
      body: JSON.stringify({ transaction_ids: transactionIds, tag_ids: tagIds }),
    }),

  bulkRemoveTags: (transactionIds: number[], tagIds: number[]) =>
    fetchJson<BulkTagsResponse>("/transactions/bulk-tags", {
      method: "DELETE",
      body: JSON.stringify({ transaction_ids: transactionIds, tag_ids: tagIds }),
    }),

  // Tag management
  createTag: (data: {
    name: string;
    parent_id?: number;
    color?: string;
    auto_patterns?: string;
  }) =>
    fetchJson<Tag>("/tags", {
      method: "POST",
      body: JSON.stringify(data),
    }),

  updateTag: (
    id: number,
    data: {
      name?: string;
      parent_id?: number | null;
      color?: string | null;
      auto_patterns?: string | null;
    }
  ) =>
    fetchJson<Tag>(`/tags/${id}`, {
      method: "PATCH",
      body: JSON.stringify(data),
    }),

  deleteTag: (id: number, reparentToParent = false) =>
    fetchJson<{ deleted_tag_id: number; transactions_moved: number; children_affected: number }>(
      `/tags/${id}?reparent_to_parent=${reparentToParent}`,
      { method: "DELETE" }
    ),

  // Tag rules
  getTagRules: () => fetchJson<TagRule[]>("/rules"),

  createTagRule: (data: {
    tag_id: number;
    pattern: string;
    pattern_type: PatternType;
    priority: number;
  }) =>
    fetchJson<TagRule>("/rules", {
      method: "POST",
      body: JSON.stringify(data),
    }),

  deleteTagRule: (id: number) =>
    fetchJson<{ success: boolean }>(`/rules/${id}`, {
      method: "DELETE",
    }),

  // Reports
  getSpendingReport: (params?: { period?: string; from?: string; to?: string; tag?: string; expand?: boolean; entity_id?: number; card_member?: string }) => {
    const searchParams = new URLSearchParams();
    if (params?.period) searchParams.set("period", params.period);
    if (params?.from) searchParams.set("from", params.from);
    if (params?.to) searchParams.set("to", params.to);
    if (params?.tag) searchParams.set("tag", params.tag);
    if (params?.expand) searchParams.set("expand", "true");
    if (params?.entity_id) searchParams.set("entity_id", params.entity_id.toString());
    if (params?.card_member) searchParams.set("card_member", params.card_member);
    const query = searchParams.toString();
    return fetchJson<SpendingSummary>(`/reports/spending${query ? `?${query}` : ""}`);
  },

  getTrendsReport: (params?: { granularity?: Granularity; period?: string; from?: string; to?: string; tag?: string; entity_id?: number; card_member?: string }) => {
    const searchParams = new URLSearchParams();
    if (params?.granularity) searchParams.set("granularity", params.granularity);
    if (params?.period) searchParams.set("period", params.period);
    if (params?.from) searchParams.set("from", params.from);
    if (params?.to) searchParams.set("to", params.to);
    if (params?.tag) searchParams.set("tag", params.tag);
    if (params?.entity_id) searchParams.set("entity_id", params.entity_id.toString());
    if (params?.card_member) searchParams.set("card_member", params.card_member);
    const query = searchParams.toString();
    return fetchJson<TrendsReport>(`/reports/trends${query ? `?${query}` : ""}`);
  },

  getMerchantsReport: (params?: { limit?: number; period?: string; from?: string; to?: string; tag?: string; entity_id?: number; card_member?: string }) => {
    const searchParams = new URLSearchParams();
    if (params?.limit) searchParams.set("limit", params.limit.toString());
    if (params?.period) searchParams.set("period", params.period);
    if (params?.from) searchParams.set("from", params.from);
    if (params?.to) searchParams.set("to", params.to);
    if (params?.tag) searchParams.set("tag", params.tag);
    if (params?.entity_id) searchParams.set("entity_id", params.entity_id.toString());
    if (params?.card_member) searchParams.set("card_member", params.card_member);
    const query = searchParams.toString();
    return fetchJson<MerchantsReport>(`/reports/merchants${query ? `?${query}` : ""}`);
  },

  getSubscriptionSummary: () => fetchJson<SubscriptionSummaryReport>("/reports/subscriptions"),

  getSavingsReport: () => fetchJson<SavingsReport>("/reports/savings"),

  // ========== Entities ==========
  getEntities: (params?: { entity_type?: EntityType; include_archived?: boolean }) => {
    const searchParams = new URLSearchParams();
    if (params?.entity_type) searchParams.set("entity_type", params.entity_type);
    if (params?.include_archived) searchParams.set("include_archived", "true");
    const query = searchParams.toString();
    return fetchJson<Entity[]>(`/entities${query ? `?${query}` : ""}`);
  },

  getEntity: (id: number) => fetchJson<Entity>(`/entities/${id}`),

  createEntity: (data: {
    name: string;
    entity_type: EntityType;
    icon?: string;
    color?: string;
  }) =>
    fetchJson<Entity>("/entities", {
      method: "POST",
      body: JSON.stringify(data),
    }),

  updateEntity: (
    id: number,
    data: {
      name?: string;
      icon?: string | null;
      color?: string | null;
    }
  ) =>
    fetchJson<Entity>(`/entities/${id}`, {
      method: "PATCH",
      body: JSON.stringify(data),
    }),

  deleteEntity: (id: number, force = false) =>
    fetchJson<{ success: boolean }>(`/entities/${id}?force=${force}`, {
      method: "DELETE",
    }),

  archiveEntity: (id: number) =>
    fetchJson<Entity>(`/entities/${id}/archive`, {
      method: "POST",
    }),

  unarchiveEntity: (id: number) =>
    fetchJson<Entity>(`/entities/${id}/unarchive`, {
      method: "POST",
    }),

  // ========== Transaction Splits ==========
  getTransactionSplits: (transactionId: number) =>
    fetchJson<TransactionSplitWithDetails[]>(`/transactions/${transactionId}/splits`),

  getSplit: (id: number) => fetchJson<TransactionSplit>(`/splits/${id}`),

  createSplit: (
    transactionId: number,
    data: {
      amount: number;
      description?: string;
      split_type?: SplitType;
      entity_id?: number;
      purchaser_id?: number;
    }
  ) =>
    fetchJson<TransactionSplit>(`/transactions/${transactionId}/splits`, {
      method: "POST",
      body: JSON.stringify(data),
    }),

  updateSplit: (
    id: number,
    data: {
      amount?: number;
      description?: string;
      split_type?: SplitType;
      entity_id?: number | null;
      purchaser_id?: number | null;
    }
  ) =>
    fetchJson<TransactionSplit>(`/splits/${id}`, {
      method: "PATCH",
      body: JSON.stringify(data),
    }),

  deleteSplit: (id: number) =>
    fetchJson<{ success: boolean }>(`/splits/${id}`, {
      method: "DELETE",
    }),

  // ========== Locations ==========
  getLocations: () => fetchJson<Location[]>("/locations"),

  getLocation: (id: number) => fetchJson<Location>(`/locations/${id}`),

  createLocation: (data: {
    name?: string;
    address?: string;
    city?: string;
    state?: string;
    country?: string;
    latitude?: number;
    longitude?: number;
    location_type?: LocationType;
  }) =>
    fetchJson<Location>("/locations", {
      method: "POST",
      body: JSON.stringify(data),
    }),

  deleteLocation: (id: number) =>
    fetchJson<{ success: boolean }>(`/locations/${id}`, {
      method: "DELETE",
    }),

  // ========== Receipts ==========
  getReceipts: (status?: ReceiptStatus) => {
    const query = status ? `?status=${status}` : "";
    return fetchJson<Receipt[]>(`/receipts${query}`);
  },

  getReceipt: (id: number) => fetchJson<Receipt>(`/receipts/${id}`),

  getTransactionReceipts: (transactionId: number) =>
    fetchJson<Receipt[]>(`/transactions/${transactionId}/receipts`),

  uploadReceipt: async (transactionId: number, imageData: Blob): Promise<ReceiptUploadResponse> => {
    const response = await fetch(`${API_BASE}/transactions/${transactionId}/receipts`, {
      method: "POST",
      headers: {
        "Content-Type": "application/octet-stream",
      },
      body: imageData,
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({ error: "Unknown error" }));
      throw new ApiError(response.status, error.error || "Upload failed");
    }

    return response.json();
  },

  uploadPendingReceipt: async (imageData: Blob): Promise<PendingReceiptResponse> => {
    const response = await fetch(`${API_BASE}/receipts`, {
      method: "POST",
      headers: {
        "Content-Type": "application/octet-stream",
      },
      body: imageData,
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({ error: "Unknown error" }));
      throw new ApiError(response.status, error.error || "Upload failed");
    }

    return response.json();
  },

  parseReceipt: (id: number) =>
    fetchJson<ReceiptParseResponse>(`/receipts/${id}/parse`, {
      method: "POST",
    }),

  linkReceipt: (receiptId: number, transactionId: number) =>
    fetchJson<Receipt>(`/receipts/${receiptId}/link`, {
      method: "POST",
      body: JSON.stringify({ transaction_id: transactionId }),
    }),

  updateReceiptStatus: (id: number, status: ReceiptStatus) =>
    fetchJson<Receipt>(`/receipts/${id}/status`, {
      method: "POST",
      body: JSON.stringify({ status }),
    }),

  unlinkReceipt: (id: number) =>
    fetchJson<Receipt>(`/receipts/${id}/unlink`, {
      method: "POST",
    }),

  deleteReceipt: (id: number) =>
    fetchJson<{ success: boolean }>(`/receipts/${id}`, {
      method: "DELETE",
    }),

  // ========== Ollama Metrics ==========
  getOllamaStats: (period?: string) => {
    const query = period ? `?period=${period}` : "";
    return fetchJson<OllamaStats>(`/ollama/stats${query}`);
  },

  getOllamaCalls: (limit?: number) => {
    const query = limit ? `?limit=${limit}` : "";
    return fetchJson<OllamaMetric[]>(`/ollama/calls${query}`);
  },

  getOllamaHealth: () => fetchJson<OllamaHealthStatus>("/ollama/health"),

  getOllamaRecommendation: () => fetchJson<ModelRecommendation>("/ollama/recommendation"),

  getOllamaModels: () => fetchJson<string[]>("/ollama/models"),

  getOllamaStatsByModel: (period?: string) => {
    const query = period ? `?period=${period}` : "";
    return fetchJson<ModelComparisonStats>(`/ollama/stats/by-model${query}`);
  },

  // Reprocess a single transaction with Ollama
  reprocessTransaction: (id: number) =>
    fetchJson<import("./types").ReprocessResponse>(`/transactions/${id}/reprocess`, {
      method: "POST",
    }),

  // Bulk reprocess multiple transactions
  bulkReprocessTransactions: (transactionIds: number[]) =>
    fetchJson<import("./types").BulkReprocessResponse>("/ollama/reprocess", {
      method: "POST",
      body: JSON.stringify({ transaction_ids: transactionIds }),
    }),

  // ========== User Feedback ==========
  getFeedback: (params?: {
    target_type?: FeedbackTargetType;
    feedback_type?: FeedbackType;
    include_reverted?: boolean;
    limit?: number;
    offset?: number;
  }) => {
    const searchParams = new URLSearchParams();
    if (params?.target_type) searchParams.set("target_type", params.target_type);
    if (params?.feedback_type) searchParams.set("feedback_type", params.feedback_type);
    if (params?.include_reverted) searchParams.set("include_reverted", "true");
    if (params?.limit) searchParams.set("limit", params.limit.toString());
    if (params?.offset) searchParams.set("offset", params.offset.toString());
    const query = searchParams.toString();
    return fetchJson<UserFeedback[]>(`/feedback${query ? `?${query}` : ""}`);
  },

  getFeedbackStats: () => fetchJson<FeedbackStats>("/feedback/stats"),

  createFeedback: (data: {
    feedback_type: FeedbackType;
    target_type: FeedbackTargetType;
    target_id?: number;
    original_value?: string;
    corrected_value?: string;
    reason?: string;
    context?: FeedbackContext;
  }) =>
    fetchJson<FeedbackResponse>("/feedback", {
      method: "POST",
      body: JSON.stringify(data),
    }),

  revertFeedback: (id: number) =>
    fetchJson<{ success: boolean }>(`/feedback/${id}/revert`, {
      method: "POST",
    }),

  // Convenience: Rate an alert's helpfulness
  rateAlert: (alertId: number, helpful: boolean, reason?: string) =>
    fetchJson<FeedbackResponse>(`/alerts/${alertId}/feedback`, {
      method: "POST",
      body: JSON.stringify({ helpful, reason }),
    }),

  getAlertFeedback: (alertId: number) =>
    fetchJson<UserFeedback[]>(`/alerts/${alertId}/feedback`),

  // Explore mode
  queryExplore: (query: string, sessionId?: string, model?: string) =>
    fetchJson<ExploreResponse>("/explore/query", {
      method: "POST",
      body: JSON.stringify({ query, session_id: sessionId, model }),
    }),

  getExploreModels: () =>
    fetchJson<ExploreModelsResponse>("/explore/models"),

  clearExploreSession: (sessionId: string) =>
    fetchJson<{ deleted: boolean }>(`/explore/session/${sessionId}`, {
      method: "DELETE",
    }),
};
