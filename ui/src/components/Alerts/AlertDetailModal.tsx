import { Ghost, TrendingUp, Users, RotateCcw, X, RefreshCw, BarChart3 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api";
import type { Alert, Transaction } from "../../types";
import { SplitsModal } from "../Transactions/SplitsModal";
import { FeedbackButton } from "../common/FeedbackButton";

interface AlertDetailModalProps {
  alert: Alert;
  onClose: () => void;
  onDismiss: (id: number) => void;
  onAcknowledge: (id: number) => void;
  onExclude?: (id: number) => void;
  onCancel?: (id: number) => void;
  onUpdate?: (alert: Alert) => void;
}

export function AlertDetailModal({ alert: initialAlert, onClose, onDismiss, onAcknowledge, onExclude, onCancel, onUpdate }: AlertDetailModalProps) {
  const [alert, setAlert] = useState(initialAlert);
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [reanalyzing, setReanalyzing] = useState(false);
  const [selectedTransaction, setSelectedTransaction] = useState<Transaction | null>(null);

  useEffect(() => {
    loadRelatedTransactions();
  }, [alert.subscription?.merchant, alert.spending_anomaly?.tag_id]);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    document.addEventListener("keydown", handleEscape);
    return () => document.removeEventListener("keydown", handleEscape);
  }, [onClose]);

  const loadRelatedTransactions = async () => {
    try {
      setLoading(true);

      if (alert.alert_type === "spending_anomaly" && alert.spending_anomaly?.tag_id) {
        // For spending anomaly alerts, load transactions in that category for current month
        const result = await api.getTransactions({
          tag_ids: [alert.spending_anomaly.tag_id],
          period: "this-month",
          limit: 20,
          sort: "date",
          order: "desc",
        });
        setTransactions(result.transactions);
      } else if (alert.subscription?.merchant) {
        // For subscription alerts, search by merchant name
        const result = await api.getTransactions({
          search: alert.subscription.merchant,
          limit: 20,
          sort: "date",
          order: "desc",
        });
        setTransactions(result.transactions);
      } else {
        setLoading(false);
        return;
      }
    } catch (err) {
      console.error("Failed to load related transactions:", err);
    } finally {
      setLoading(false);
    }
  };

  const getIcon = () => {
    switch (alert.alert_type) {
      case "zombie":
        return <Ghost className="w-6 h-6 text-attention" />;
      case "price_increase":
        return <TrendingUp className="w-6 h-6 text-waste" />;
      case "duplicate":
        return <Users className="w-6 h-6 text-hone-500" />;
      case "resume":
        return <RotateCcw className="w-6 h-6 text-waste" />;
      case "spending_anomaly":
        return <BarChart3 className="w-6 h-6 text-attention" />;
    }
  };

  const getLabel = () => {
    switch (alert.alert_type) {
      case "zombie":
        return "Zombie Subscription";
      case "price_increase":
        return "Price Increase";
      case "duplicate":
        return "Duplicate Service";
      case "resume":
        return "Subscription Resumed";
      case "spending_anomaly":
        return "Spending Change";
    }
  };

  const getDescription = () => {
    switch (alert.alert_type) {
      case "zombie":
        return "This subscription appears to be recurring but hasn't been acknowledged. Review the transactions below to decide if you still need it.";
      case "price_increase":
        return "The price of this subscription has increased. Review the transaction history to see the change.";
      case "duplicate":
        return "You may have multiple subscriptions for similar services. Consider whether you need all of them.";
      case "resume":
        return "This subscription that was previously cancelled has resumed. Review if this was intentional.";
      case "spending_anomaly":
        return "Your spending in this category changed significantly compared to your 3-month average.";
    }
  };

  // Parse date-only strings as local dates to avoid timezone shift
  const formatDate = (dateStr: string) => {
    const [year, month, day] = dateStr.split("-").map(Number);
    return new Date(year, month - 1, day, 12, 0, 0).toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "2-digit",
    });
  };

  const handleDismiss = () => {
    onDismiss(alert.id);
    onClose();
  };

  const handleAcknowledge = () => {
    if (alert.subscription_id) {
      onAcknowledge(alert.subscription_id);
      onClose();
    }
  };

  const handleExclude = async () => {
    if (alert.id) {
      try {
        // Use the combined dismiss-exclude endpoint that handles both
        await api.dismissAlertExclude(alert.id);
        // Notify parent to refresh subscriptions state
        if (onExclude && alert.subscription_id) {
          onExclude(alert.subscription_id);
        }
        onClose();
      } catch (err) {
        console.error("Failed to exclude subscription:", err);
      }
    }
  };

  const handleCancel = async () => {
    if (alert.subscription_id) {
      try {
        await api.cancelSubscription(alert.subscription_id);
        // Dismiss the alert since we've handled it
        onDismiss(alert.id);
        // Notify parent to update subscription state
        if (onCancel) {
          onCancel(alert.subscription_id);
        }
        onClose();
      } catch (err) {
        console.error("Failed to cancel subscription:", err);
      }
    }
  };

  const handleReanalyze = async () => {
    try {
      setReanalyzing(true);
      const updatedAlert = await api.reanalyzeSpendingAlert(alert.id);
      setAlert(updatedAlert);
      if (onUpdate) {
        onUpdate(updatedAlert);
      }
    } catch (err) {
      console.error("Failed to reanalyze:", err);
    } finally {
      setReanalyzing(false);
    }
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

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={onClose}>
      <div
        className="card w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto animate-slide-up"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="card-header flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="flex-shrink-0 p-2 bg-hone-100 dark:bg-hone-700 rounded-full">
              {getIcon()}
            </div>
            <div>
              <h2 className="text-lg font-semibold text-hone-900 dark:text-hone-50">{getLabel()}</h2>
              {alert.subscription && (
                <p className="text-sm text-hone-600 dark:text-hone-300">
                  {alert.subscription.merchant}
                </p>
              )}
            </div>
          </div>
          <button onClick={onClose} className="p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200">
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="card-body space-y-6">
          {/* Description */}
          <p className="text-sm text-hone-500 dark:text-hone-400">
            {getDescription()}
          </p>

          {/* Subscription Details */}
          {alert.subscription && (
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 p-4 bg-hone-50 dark:bg-hone-800/50 rounded-lg">
              <div>
                <p className="text-xs text-hone-400 uppercase tracking-wide">Amount</p>
                <p className="text-lg font-semibold text-hone-900 dark:text-hone-50">
                  ${alert.subscription.amount?.toFixed(2) || "—"}
                  {alert.subscription.frequency && (
                    <span className="text-sm text-hone-500 dark:text-hone-400">
                      /{alert.subscription.frequency.replace("ly", "")}
                    </span>
                  )}
                </p>
              </div>
              <div>
                <p className="text-xs text-hone-400 uppercase tracking-wide">Status</p>
                <p className={`text-lg font-semibold capitalize ${
                  alert.subscription.status === "active"
                    ? "text-savings"
                    : alert.subscription.status === "cancelled"
                      ? "text-hone-500"
                      : "text-attention"
                }`}>
                  {alert.subscription.status}
                </p>
              </div>
              <div>
                <p className="text-xs text-hone-400 uppercase tracking-wide">First Seen</p>
                <p className="text-hone-900 dark:text-hone-50">
                  {alert.subscription.first_seen
                    ? formatDate(alert.subscription.first_seen)
                    : "—"}
                </p>
              </div>
              <div>
                <p className="text-xs text-hone-400 uppercase tracking-wide">Last Seen</p>
                <p className="text-hone-900 dark:text-hone-50">
                  {alert.subscription.last_seen
                    ? formatDate(alert.subscription.last_seen)
                    : "—"}
                </p>
              </div>
            </div>
          )}

          {/* Alert Message */}
          {alert.message && (
            <div className="p-4 bg-attention/10 dark:bg-attention/20 border border-attention/30 rounded-lg">
              <p className="text-sm text-hone-700 dark:text-hone-200">{alert.message}</p>
            </div>
          )}

          {/* Ollama Analysis (for duplicate alerts) */}
          {alert.alert_type === "duplicate" && alert.ollama_analysis && (
            <div className="p-4 bg-hone-50 dark:bg-hone-800/50 rounded-lg space-y-3">
              <div>
                <p className="text-xs text-hone-400 uppercase tracking-wide mb-1">What they have in common</p>
                <p className="text-sm text-hone-700 dark:text-hone-200">{alert.ollama_analysis.overlap}</p>
              </div>
              {alert.ollama_analysis.unique_features.length > 0 && (
                <div>
                  <p className="text-xs text-hone-400 uppercase tracking-wide mb-2">What makes each unique</p>
                  <div className="space-y-2">
                    {alert.ollama_analysis.unique_features.map((feature) => (
                      <div key={feature.service} className="flex gap-2">
                        <span className="font-medium text-sm text-hone-900 dark:text-hone-100 whitespace-nowrap">
                          {feature.service}:
                        </span>
                        <span className="text-sm text-hone-600 dark:text-hone-300">
                          {feature.unique}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              <div className="pt-2 border-t border-hone-200 dark:border-hone-700">
                <FeedbackButton
                  targetType="explanation"
                  targetId={alert.id}
                />
              </div>
            </div>
          )}

          {/* Spending Anomaly Details */}
          {alert.alert_type === "spending_anomaly" && alert.spending_anomaly && (
            <div className="space-y-4">
              {/* Spending Change Stats */}
              <div className="grid grid-cols-3 gap-4 p-4 bg-hone-50 dark:bg-hone-800/50 rounded-lg">
                <div>
                  <p className="text-xs text-hone-400 uppercase tracking-wide">Category</p>
                  <p className="text-lg font-semibold text-hone-900 dark:text-hone-50">
                    {alert.spending_anomaly.tag_name}
                  </p>
                </div>
                <div>
                  <p className="text-xs text-hone-400 uppercase tracking-wide">Baseline</p>
                  <p className="text-lg font-semibold text-hone-600 dark:text-hone-300">
                    ${alert.spending_anomaly.baseline_amount.toFixed(2)}/mo
                  </p>
                </div>
                <div>
                  <p className="text-xs text-hone-400 uppercase tracking-wide">Current</p>
                  <p className={`text-lg font-semibold ${
                    alert.spending_anomaly.percent_change > 0 ? "text-waste" : "text-savings"
                  }`}>
                    ${alert.spending_anomaly.current_amount.toFixed(2)}
                    <span className="text-sm ml-1">
                      ({alert.spending_anomaly.percent_change > 0 ? "+" : ""}
                      {alert.spending_anomaly.percent_change.toFixed(0)}%)
                    </span>
                  </p>
                </div>
              </div>

              {/* AI Analysis */}
              <div className="p-4 bg-hone-50 dark:bg-hone-800/50 rounded-lg space-y-3">
                {alert.spending_anomaly.explanation ? (
                  <>
                    <p className="text-sm font-medium text-hone-900 dark:text-hone-100">
                      {alert.spending_anomaly.explanation.summary}
                    </p>
                    <ul className="space-y-1">
                      {alert.spending_anomaly.explanation.reasons.map((reason, i) => (
                        <li key={i} className="text-sm text-hone-600 dark:text-hone-300 flex items-start gap-2">
                          <span className="text-hone-400">•</span>
                          {reason}
                        </li>
                      ))}
                    </ul>
                    <div className="flex items-center justify-between pt-2 border-t border-hone-200 dark:border-hone-700">
                      <span className="text-xs text-hone-500">
                        Analyzed by {alert.spending_anomaly.explanation.model} on{" "}
                        {formatDateTime(alert.spending_anomaly.explanation.analyzed_at)}
                      </span>
                      <div className="flex items-center gap-3">
                        <FeedbackButton
                          targetType="explanation"
                          targetId={alert.id}
                        />
                        <button
                          onClick={handleReanalyze}
                          disabled={reanalyzing}
                          className="text-sm text-hone-600 hover:text-hone-800 dark:text-hone-400 dark:hover:text-hone-200 flex items-center gap-1"
                        >
                          <RefreshCw className={`w-3 h-3 ${reanalyzing ? "animate-spin" : ""}`} />
                          Re-analyze
                        </button>
                      </div>
                    </div>
                  </>
                ) : (
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-hone-500">No AI analysis available</span>
                    <button
                      onClick={handleReanalyze}
                      disabled={reanalyzing}
                      className="text-sm text-hone-600 hover:text-hone-800 dark:text-hone-400 dark:hover:text-hone-200 flex items-center gap-1"
                    >
                      <RefreshCw className={`w-3 h-3 ${reanalyzing ? "animate-spin" : ""}`} />
                      {reanalyzing ? "Analyzing..." : "Analyze with Ollama"}
                    </button>
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Related Transactions */}
          <div>
            <h3 className="text-sm font-medium text-hone-500 dark:text-hone-400 uppercase tracking-wide mb-3">
              Related Transactions
            </h3>
            {loading ? (
              <div className="flex items-center justify-center py-8">
                <RefreshCw className="w-6 h-6 text-hone-400 animate-spin" />
              </div>
            ) : transactions.length === 0 ? (
              <p className="text-center py-8 text-hone-500 dark:text-hone-400">
                No matching transactions found
              </p>
            ) : (
              <div className="border border-hone-200 dark:border-hone-700 rounded-lg divide-y divide-hone-200 dark:divide-hone-700 max-h-64 overflow-y-auto">
                {transactions.map((tx) => {
                  const isExpense = tx.amount < 0;
                  return (
                    <div
                      key={tx.id}
                      onClick={() => setSelectedTransaction(tx)}
                      className="flex items-center justify-between px-4 py-3 hover:bg-hone-50 dark:hover:bg-hone-700 cursor-pointer"
                    >
                      <div className="flex-1 min-w-0">
                        <p className="font-medium text-hone-900 dark:text-hone-100 truncate">
                          {tx.merchant_normalized || tx.description}
                        </p>
                        <p className="text-sm text-hone-500 dark:text-hone-400">
                          {formatDate(tx.date)}
                        </p>
                      </div>
                      <div className={`font-semibold ml-4 ${isExpense ? "text-hone-600 dark:text-hone-300" : "text-savings-dark"}`}>
                        {isExpense ? "-" : "+"}${Math.abs(tx.amount).toFixed(2)}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {/* Actions */}
          {!alert.dismissed && (
            <div className="flex items-center justify-end gap-3 pt-4 border-t border-hone-200 dark:border-hone-700">
              {alert.subscription_id && alert.alert_type === "zombie" && (
                <>
                  <button
                    onClick={handleAcknowledge}
                    className="btn-success"
                  >
                    I still use this
                  </button>
                  <button
                    onClick={handleCancel}
                    className="btn-primary"
                    title="Mark this subscription as cancelled. Tracks savings."
                  >
                    I cancelled this
                  </button>
                  {onExclude && (
                    <button
                      onClick={handleExclude}
                      className="btn-secondary"
                      title="This is not a subscription (e.g., grocery store). Won't be flagged again."
                    >
                      Not a subscription
                    </button>
                  )}
                </>
              )}
              <button onClick={handleDismiss} className="btn-secondary">
                Dismiss Alert
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Transaction detail modal */}
      {selectedTransaction && (
        <SplitsModal
          transaction={selectedTransaction}
          onClose={() => setSelectedTransaction(null)}
        />
      )}
    </div>
  );
}
