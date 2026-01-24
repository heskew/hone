import { AlertTriangle, Ban, Calendar, CheckCircle, CreditCard, Ghost, RefreshCw, TrendingUp, X } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api";
import type { Alert, Subscription, Transaction } from "../../types";
import { SplitsModal } from "../Transactions/SplitsModal";

interface SubscriptionDetailModalProps {
  subscription: Subscription;
  accountName: string | null;
  alerts: Alert[];
  onClose: () => void;
  onAcknowledge: (id: number) => void;
  onCancel: (id: number) => void;
  onExclude: (id: number) => void;
  onUnexclude: (id: number) => void;
}

type TabType = "overview" | "alerts" | "transactions";

export function SubscriptionDetailModal({
  subscription,
  accountName,
  alerts,
  onClose,
  onAcknowledge,
  onCancel,
  onExclude,
  onUnexclude,
}: SubscriptionDetailModalProps) {
  const [activeTab, setActiveTab] = useState<TabType>("overview");
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [loadingTransactions, setLoadingTransactions] = useState(false);
  const [actionInProgress, setActionInProgress] = useState(false);
  const [selectedTransaction, setSelectedTransaction] = useState<Transaction | null>(null);

  // Filter alerts related to this subscription
  const relatedAlerts = alerts.filter((a) => a.subscription_id === subscription.id);

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

  // Load related transactions when switching to transactions tab
  useEffect(() => {
    if (activeTab === "transactions" && transactions.length === 0) {
      loadTransactions();
    }
  }, [activeTab]);

  const loadTransactions = async () => {
    try {
      setLoadingTransactions(true);
      // Search for transactions matching this merchant
      const response = await api.getTransactions({
        search: subscription.merchant,
        limit: 50,
        sort: "date",
        order: "desc",
      });
      setTransactions(response.transactions);
    } catch (err) {
      console.error("Failed to load transactions:", err);
    } finally {
      setLoadingTransactions(false);
    }
  };

  const formatFrequency = (freq: string | null) => {
    switch (freq) {
      case "weekly":
        return "Weekly";
      case "monthly":
        return "Monthly";
      case "yearly":
        return "Yearly";
      default:
        return "Unknown";
    }
  };

  const formatFrequencySuffix = (freq: string | null) => {
    switch (freq) {
      case "weekly":
        return "/week";
      case "monthly":
        return "/mo";
      case "yearly":
        return "/year";
      default:
        return "";
    }
  };

  const getStatusBadge = () => {
    switch (subscription.status) {
      case "zombie":
        return (
          <span className="inline-flex items-center gap-1 px-2 py-1 rounded-full text-sm font-medium bg-attention/10 text-attention">
            <Ghost className="w-4 h-4" />
            Zombie
          </span>
        );
      case "active":
        return (
          <span className="inline-flex items-center gap-1 px-2 py-1 rounded-full text-sm font-medium bg-savings/10 text-savings">
            <CheckCircle className="w-4 h-4" />
            Active
          </span>
        );
      case "cancelled":
        return (
          <span className="inline-flex items-center gap-1 px-2 py-1 rounded-full text-sm font-medium bg-hone-500/10 text-hone-500">
            <Ban className="w-4 h-4" />
            Cancelled
          </span>
        );
      case "excluded":
        return (
          <span className="inline-flex items-center gap-1 px-2 py-1 rounded-full text-sm font-medium bg-hone-400/10 text-hone-400">
            <Ban className="w-4 h-4" />
            Excluded
          </span>
        );
      default:
        return null;
    }
  };

  const getAlertTypeBadge = (alertType: string) => {
    switch (alertType) {
      case "zombie":
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-attention/10 text-attention">
            <Ghost className="w-3 h-3" />
            Zombie
          </span>
        );
      case "price_increase":
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-waste/10 text-waste">
            <TrendingUp className="w-3 h-3" />
            Price Increase
          </span>
        );
      case "duplicate":
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-attention/10 text-attention">
            <AlertTriangle className="w-3 h-3" />
            Duplicate
          </span>
        );
      case "resume":
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-attention/10 text-attention">
            <RefreshCw className="w-3 h-3" />
            Resumed
          </span>
        );
      default:
        return null;
    }
  };

  const handleAcknowledge = async () => {
    setActionInProgress(true);
    try {
      await api.acknowledgeSubscription(subscription.id);
      onAcknowledge(subscription.id);
      onClose();
    } catch (err) {
      console.error("Failed to acknowledge:", err);
    } finally {
      setActionInProgress(false);
    }
  };

  const handleCancel = async () => {
    setActionInProgress(true);
    try {
      await api.cancelSubscription(subscription.id);
      onCancel(subscription.id);
      onClose();
    } catch (err) {
      console.error("Failed to cancel:", err);
    } finally {
      setActionInProgress(false);
    }
  };

  const handleExclude = async () => {
    setActionInProgress(true);
    try {
      await api.excludeSubscription(subscription.id);
      onExclude(subscription.id);
      onClose();
    } catch (err) {
      console.error("Failed to exclude:", err);
    } finally {
      setActionInProgress(false);
    }
  };

  const handleUnexclude = async () => {
    setActionInProgress(true);
    try {
      await api.unexcludeSubscription(subscription.id);
      onUnexclude(subscription.id);
      onClose();
    } catch (err) {
      console.error("Failed to unexclude:", err);
    } finally {
      setActionInProgress(false);
    }
  };

  const tabClass = (tab: TabType) =>
    `px-3 py-2 text-sm font-medium border-b-2 transition-colors ${
      activeTab === tab
        ? "border-hone-700 text-hone-900 dark:text-hone-100"
        : "border-transparent text-hone-500 hover:text-hone-700 dark:text-hone-400 dark:hover:text-hone-200"
    }`;

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      onClick={onClose}
    >
      <div
        className="card w-full max-w-2xl mx-4 max-h-[90vh] flex flex-col animate-slide-up"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="card-header flex items-start justify-between">
          <div>
            <h2 className="text-lg font-semibold text-hone-900 dark:text-hone-100">
              {subscription.merchant}
            </h2>
            <div className="flex items-center gap-2 mt-1">
              {getStatusBadge()}
              {subscription.user_acknowledged && (
                <span className="badge-success text-xs" title={subscription.acknowledged_at ? `Acknowledged ${new Date(subscription.acknowledged_at).toLocaleDateString()}` : undefined}>
                  Acknowledged
                  {subscription.acknowledged_at && (
                    <span className="ml-1 opacity-75">
                      ({new Date(subscription.acknowledged_at).toLocaleDateString("en-US", { month: "short", day: "numeric" })})
                    </span>
                  )}
                </span>
              )}
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
            <button className={tabClass("alerts")} onClick={() => setActiveTab("alerts")}>
              Alerts
              {relatedAlerts.length > 0 && (
                <span className="ml-1.5 px-1.5 py-0.5 text-xs rounded-full bg-attention/20 text-attention">
                  {relatedAlerts.length}
                </span>
              )}
            </button>
            <button className={tabClass("transactions")} onClick={() => setActiveTab("transactions")}>
              Transactions
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="card-body overflow-y-auto flex-1">
          {activeTab === "overview" && (
            <div className="space-y-6">
              {/* Subscription Details */}
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <div className="text-sm text-hone-500 dark:text-hone-400">Amount</div>
                  <div className="text-lg font-semibold text-hone-900 dark:text-hone-100">
                    {subscription.amount ? (
                      <>
                        <span className="amount-negative">
                          ${subscription.amount.toFixed(2)}
                        </span>
                        <span className="text-hone-400 text-sm font-normal ml-1">
                          {formatFrequencySuffix(subscription.frequency)}
                        </span>
                      </>
                    ) : (
                      <span className="text-hone-400">Unknown</span>
                    )}
                  </div>
                </div>

                <div>
                  <div className="text-sm text-hone-500 dark:text-hone-400">Frequency</div>
                  <div className="text-lg font-semibold text-hone-900 dark:text-hone-100">
                    {formatFrequency(subscription.frequency)}
                  </div>
                </div>

                <div>
                  <div className="text-sm text-hone-500 dark:text-hone-400">First Seen</div>
                  <div className="flex items-center gap-2 text-hone-900 dark:text-hone-100">
                    <Calendar className="w-4 h-4 text-hone-400" />
                    {subscription.first_seen
                      ? (() => {
                          const [y, m, d] = subscription.first_seen.split("-").map(Number);
                          return new Date(y, m - 1, d, 12, 0, 0).toLocaleDateString("en-US", {
                            month: "short",
                            day: "numeric",
                            year: "numeric",
                          });
                        })()
                      : "Unknown"}
                  </div>
                </div>

                <div>
                  <div className="text-sm text-hone-500 dark:text-hone-400">Last Seen</div>
                  <div className="flex items-center gap-2 text-hone-900 dark:text-hone-100">
                    <Calendar className="w-4 h-4 text-hone-400" />
                    {subscription.last_seen
                      ? (() => {
                          const [y, m, d] = subscription.last_seen.split("-").map(Number);
                          return new Date(y, m - 1, d, 12, 0, 0).toLocaleDateString("en-US", {
                            month: "short",
                            day: "numeric",
                            year: "numeric",
                          });
                        })()
                      : "Unknown"}
                  </div>
                </div>

                {accountName && (
                  <div className="col-span-2">
                    <div className="text-sm text-hone-500 dark:text-hone-400">Account</div>
                    <div className="flex items-center gap-2 text-hone-900 dark:text-hone-100">
                      <CreditCard className="w-4 h-4 text-hone-400" />
                      {accountName}
                    </div>
                  </div>
                )}
              </div>

              {/* Actions */}
              <div className="border-t border-hone-200 dark:border-hone-700 pt-4">
                <h3 className="text-sm font-medium text-hone-700 dark:text-hone-300 mb-3">
                  Actions
                </h3>
                <div className="flex flex-wrap gap-2">
                  {subscription.status === "zombie" && !subscription.user_acknowledged && (
                    <button
                      onClick={handleAcknowledge}
                      disabled={actionInProgress}
                      className="btn-primary text-sm"
                    >
                      {actionInProgress ? "..." : "I still use this"}
                    </button>
                  )}

                  {subscription.status === "active" && !subscription.user_acknowledged && (
                    <button
                      onClick={handleAcknowledge}
                      disabled={actionInProgress}
                      className="btn-primary text-sm"
                    >
                      {actionInProgress ? "..." : "Acknowledge"}
                    </button>
                  )}

                  {subscription.status === "active" && subscription.user_acknowledged && (
                    <button
                      onClick={handleAcknowledge}
                      disabled={actionInProgress}
                      className="btn-secondary text-sm"
                      title="Refresh acknowledgment to prevent future zombie alerts"
                    >
                      {actionInProgress ? "..." : "Re-acknowledge"}
                    </button>
                  )}

                  {(subscription.status === "active" || subscription.status === "zombie") && (
                    <button
                      onClick={handleCancel}
                      disabled={actionInProgress}
                      className="btn-secondary text-sm"
                    >
                      {actionInProgress ? "..." : "I cancelled this"}
                    </button>
                  )}

                  {subscription.status !== "excluded" && (
                    <button
                      onClick={handleExclude}
                      disabled={actionInProgress}
                      className="btn-ghost text-sm text-hone-500"
                    >
                      {actionInProgress ? "..." : "Not a subscription"}
                    </button>
                  )}

                  {subscription.status === "excluded" && (
                    <button
                      onClick={handleUnexclude}
                      disabled={actionInProgress}
                      className="btn-secondary text-sm"
                    >
                      {actionInProgress ? "..." : "Include again"}
                    </button>
                  )}
                </div>
              </div>
            </div>
          )}

          {activeTab === "alerts" && (
            <div className="space-y-3">
              {relatedAlerts.length === 0 ? (
                <div className="text-center py-8 text-hone-500 dark:text-hone-400">
                  No alerts for this subscription
                </div>
              ) : (
                relatedAlerts.map((alert) => (
                  <div
                    key={alert.id}
                    className={`p-3 rounded-lg border ${
                      alert.dismissed
                        ? "bg-hone-50 dark:bg-hone-800 border-hone-200 dark:border-hone-700 opacity-60"
                        : "bg-white dark:bg-hone-900 border-hone-200 dark:border-hone-700"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div>
                        {getAlertTypeBadge(alert.alert_type)}
                        {alert.message && (
                          <p className="mt-1 text-sm text-hone-700 dark:text-hone-300">
                            {alert.message}
                          </p>
                        )}
                        <p className="mt-1 text-xs text-hone-400">
                          {new Date(alert.created_at).toLocaleDateString("en-US", {
                            month: "short",
                            day: "numeric",
                            year: "numeric",
                          })}
                        </p>
                      </div>
                      {alert.dismissed && (
                        <span className="text-xs text-hone-400">Dismissed</span>
                      )}
                    </div>
                  </div>
                ))
              )}
            </div>
          )}

          {activeTab === "transactions" && (
            <div className="space-y-2">
              {loadingTransactions ? (
                <div className="flex items-center justify-center py-8">
                  <RefreshCw className="w-5 h-5 animate-spin text-hone-400" />
                </div>
              ) : transactions.length === 0 ? (
                <div className="text-center py-8 text-hone-500 dark:text-hone-400">
                  No transactions found for this merchant
                </div>
              ) : (
                <>
                  <p className="text-sm text-hone-500 dark:text-hone-400 mb-3">
                    {transactions.length} transaction{transactions.length !== 1 ? "s" : ""} found
                  </p>
                  <div className="divide-y divide-hone-100 dark:divide-hone-800">
                    {transactions.map((tx) => (
                      <div
                        key={tx.id}
                        onClick={() => setSelectedTransaction(tx)}
                        className="py-2 flex items-center justify-between cursor-pointer hover:bg-hone-50 dark:hover:bg-hone-800 -mx-2 px-2 rounded"
                      >
                        <div>
                          <div className="text-sm text-hone-900 dark:text-hone-100">
                            {(() => {
                              const [y, m, d] = tx.date.split("-").map(Number);
                              return new Date(y, m - 1, d, 12, 0, 0).toLocaleDateString("en-US", {
                                month: "short",
                                day: "numeric",
                                year: "numeric",
                              });
                            })()}
                          </div>
                          <div className="text-xs text-hone-400 truncate max-w-[200px]">
                            {tx.merchant_normalized || tx.description}
                          </div>
                        </div>
                        <div className="text-hone-600 dark:text-hone-300 font-medium">
                          ${Math.abs(tx.amount).toFixed(2)}
                        </div>
                      </div>
                    ))}
                  </div>
                </>
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="card-footer flex justify-end flex-shrink-0">
          <button onClick={onClose} className="btn-secondary">
            Close
          </button>
        </div>
      </div>

      {selectedTransaction && (
        <SplitsModal
          transaction={selectedTransaction}
          onClose={() => setSelectedTransaction(null)}
        />
      )}
    </div>
  );
}
