import { useState, useEffect } from "react";
import { RefreshCw, TrendingUp, Ghost, Copy, TrendingDown, Check, X } from "lucide-react";
import { api } from "../../api";
import type { SubscriptionSummaryReport, SavingsReport, Alert } from "../../types";

export function SubscriptionsTab() {
  const [subData, setSubData] = useState<SubscriptionSummaryReport | null>(null);
  const [savingsData, setSavingsData] = useState<SavingsReport | null>(null);
  const [alerts, setAlerts] = useState<Alert[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadData = async () => {
    try {
      setLoading(true);
      setError(null);
      const [subs, savings, alertsData] = await Promise.all([
        api.getSubscriptionSummary(),
        api.getSavingsReport(),
        api.getAlerts(),
      ]);
      setSubData(subs);
      setSavingsData(savings);
      setAlerts(alertsData);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load subscription data");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleAcknowledge = async (subscriptionId: number) => {
    try {
      await api.acknowledgeSubscription(subscriptionId);
      // Dismiss related alerts
      const relatedAlerts = alerts.filter((a) => a.subscription_id === subscriptionId);
      for (const alert of relatedAlerts) {
        await api.dismissAlert(alert.id);
      }
      // Reload data
      await loadData();
    } catch (err) {
      console.error("Failed to acknowledge subscription:", err);
    }
  };

  const handleCancel = async (subscriptionId: number) => {
    try {
      await api.cancelSubscription(subscriptionId);
      // Dismiss related alerts
      const relatedAlerts = alerts.filter((a) => a.subscription_id === subscriptionId);
      for (const alert of relatedAlerts) {
        await api.dismissAlert(alert.id);
      }
      // Reload data
      await loadData();
    } catch (err) {
      console.error("Failed to cancel subscription:", err);
    }
  };

  if (loading) {
    return (
      <div className="card p-6 flex items-center justify-center">
        <RefreshCw className="w-5 h-5 animate-spin text-hone-400" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="card p-6">
        <p className="text-waste">{error}</p>
      </div>
    );
  }

  if (!subData) {
    return (
      <div className="card p-6 text-center">
        <p className="text-hone-500">No subscription data available</p>
      </div>
    );
  }

  // Group subscriptions by status
  const activeSubscriptions = subData.subscriptions.filter((s) => s.status === "active");
  const zombieSubscriptions = subData.subscriptions.filter((s) => s.status === "zombie");
  const cancelledSubscriptions = subData.subscriptions.filter((s) => s.status === "cancelled");

  // Get duplicate alerts
  const duplicateAlerts = alerts.filter((a) => a.alert_type === "duplicate" && !a.dismissed);

  // Get price increase alerts
  const priceIncreaseAlerts = alerts.filter((a) => a.alert_type === "price_increase" && !a.dismissed);

  const formatFrequency = (freq: string) => {
    switch (freq) {
      case "monthly": return "/mo";
      case "yearly": return "/yr";
      case "weekly": return "/wk";
      default: return "";
    }
  };

  return (
    <div className="space-y-6">
      {/* Summary cards */}
      <div className="grid grid-cols-4 gap-4">
        <div className="card p-4">
          <div className="text-sm text-hone-500">Active Subscriptions</div>
          <div className="text-2xl font-bold text-hone-900 dark:text-hone-100">{activeSubscriptions.length}</div>
        </div>
        <div className="card p-4">
          <div className="text-sm text-hone-500">Monthly Cost</div>
          <div className="text-2xl font-bold text-hone-900 dark:text-hone-100">
            ${subData.total_monthly.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
          </div>
        </div>
        <div className="card p-4">
          <div className="text-sm text-hone-500">Potential Waste</div>
          <div className="text-2xl font-bold text-waste">
            ${subData.waste.total_waste_monthly.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}/mo
          </div>
        </div>
        <div className="card p-4">
          <div className="text-sm text-hone-500">Cancelled</div>
          <div className="text-2xl font-bold text-hone-900 dark:text-hone-100">{cancelledSubscriptions.length}</div>
        </div>
      </div>

      {/* Savings celebration */}
      {savingsData && savingsData.total_savings > 0 && (
        <div className="card p-6 bg-gradient-to-r from-savings/10 to-savings/5 border-savings/20">
          <div className="flex items-center gap-4">
            <div className="w-12 h-12 rounded-full bg-savings/20 flex items-center justify-center">
              <TrendingUp className="w-6 h-6 text-savings" />
            </div>
            <div>
              <div className="text-lg font-semibold text-savings">
                You've saved ${savingsData.total_savings.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}!
              </div>
              <div className="text-sm text-hone-600">
                By cancelling {savingsData.cancelled_count} subscription{savingsData.cancelled_count !== 1 ? "s" : ""}, you save ${savingsData.total_monthly_saved.toLocaleString(undefined, { minimumFractionDigits: 2 })}/month
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Zombies Section */}
      {zombieSubscriptions.length > 0 && (
        <div className="card">
          <div className="card-header bg-red-50 border-b border-red-100">
            <div className="flex items-center gap-2">
              <Ghost className="w-5 h-5 text-waste" />
              <h3 className="font-semibold text-waste">Zombie Subscriptions</h3>
              <span className="text-sm text-waste/70">
                ({zombieSubscriptions.length} detected - ${subData.waste.zombie_monthly.toFixed(2)}/mo)
              </span>
            </div>
            <p className="text-sm text-hone-500 mt-1">
              These recurring charges haven't been acknowledged. Are you still using them?
            </p>
          </div>
          <div className="divide-y divide-hone-100">
            {zombieSubscriptions.map((sub) => {
              // Parse date-only strings as local dates to avoid timezone shift
              const parseLocalDate = (dateStr: string) => {
                const [year, month, day] = dateStr.split("-").map(Number);
                return new Date(year, month - 1, day, 12, 0, 0);
              };

              // Calculate if the subscription pattern appears broken (missed expected charges)
              const lastSeenDate = parseLocalDate(sub.last_seen);
              const now = new Date();
              const daysSinceLastSeen = Math.floor((now.getTime() - lastSeenDate.getTime()) / (1000 * 60 * 60 * 24));
              const expectedInterval = sub.frequency === "monthly" ? 45 : sub.frequency === "weekly" ? 14 : 400;
              const likelyCancelled = daysSinceLastSeen > expectedInterval;

              return (
                <div key={sub.id} className="px-4 py-3 flex items-center justify-between hover:bg-hone-50 dark:hover:bg-hone-800 dark:hover:bg-hone-800">
                  <div>
                    <div className="font-medium text-hone-900 dark:text-hone-100">{sub.merchant}</div>
                    <div className="text-sm text-hone-500">
                      Since {parseLocalDate(sub.first_seen).toLocaleDateString("en-US", { month: "short", year: "numeric" })}
                      {" · "}Last seen {parseLocalDate(sub.last_seen).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "2-digit" })}
                      {likelyCancelled && (
                        <span className="ml-2 text-attention">
                          (no charge in {daysSinceLastSeen} days)
                        </span>
                      )}
                    </div>
                  </div>
                  <div className="flex items-center gap-3">
                    <div className="text-right">
                      <div className="font-semibold text-hone-900 dark:text-hone-100">
                        ${sub.amount.toFixed(2)}{formatFrequency(sub.frequency)}
                      </div>
                    </div>
                    <button
                      onClick={() => handleAcknowledge(sub.id)}
                      className="btn-secondary text-sm px-3 py-1.5"
                    >
                      <Check className="w-4 h-4 mr-1" />
                      I use this
                    </button>
                    <button
                      onClick={() => handleCancel(sub.id)}
                      className="btn-ghost text-sm px-3 py-1.5 text-hone-500 hover:text-waste"
                      title="Mark as cancelled"
                    >
                      <X className="w-4 h-4 mr-1" />
                      Cancelled
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Duplicates Section */}
      {duplicateAlerts.length > 0 && (
        <div className="card">
          <div className="card-header bg-amber-50 border-b border-amber-100">
            <div className="flex items-center gap-2">
              <Copy className="w-5 h-5 text-attention" />
              <h3 className="font-semibold text-attention">Duplicate Services</h3>
              <span className="text-sm text-attention/70">
                ({subData.waste.duplicate_count} detected - ${subData.waste.duplicate_monthly.toFixed(2)}/mo)
              </span>
            </div>
            <p className="text-sm text-hone-500 mt-1">
              You may be paying for multiple services that do the same thing.
            </p>
          </div>
          <div className="divide-y divide-hone-100">
            {duplicateAlerts.map((alert) => (
              <div key={alert.id} className="px-4 py-3 hover:bg-hone-50 dark:hover:bg-hone-800">
                <div className="text-hone-900 dark:text-hone-100">{alert.message}</div>
                {alert.subscription && (
                  <div className="text-sm text-hone-500 mt-1">
                    {alert.subscription.merchant} - ${alert.subscription.amount?.toFixed(2)}{formatFrequency(alert.subscription.frequency || "")}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Price Increases Section */}
      {priceIncreaseAlerts.length > 0 && (
        <div className="card">
          <div className="card-header bg-purple-50 border-b border-purple-100">
            <div className="flex items-center gap-2">
              <TrendingDown className="w-5 h-5 text-purple-600" />
              <h3 className="font-semibold text-purple-600">Price Increases</h3>
              <span className="text-sm text-purple-600/70">
                ({subData.waste.price_increase_count} detected - +${subData.waste.price_increase_delta.toFixed(2)}/mo)
              </span>
            </div>
            <p className="text-sm text-hone-500 mt-1">
              These services have quietly raised their prices.
            </p>
          </div>
          <div className="divide-y divide-hone-100">
            {priceIncreaseAlerts.map((alert) => (
              <div key={alert.id} className="px-4 py-3 hover:bg-hone-50 dark:hover:bg-hone-800">
                <div className="text-hone-900 dark:text-hone-100">{alert.message}</div>
                {alert.subscription && (
                  <div className="text-sm text-hone-500 mt-1">
                    Now ${alert.subscription.amount?.toFixed(2)}{formatFrequency(alert.subscription.frequency || "")}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Active Subscriptions */}
      {activeSubscriptions.length > 0 && (
        <div className="card">
          <div className="card-header">
            <h3 className="font-semibold">Active Subscriptions</h3>
            <p className="text-sm text-hone-500">Subscriptions you've acknowledged as intentional</p>
          </div>
          <table className="w-full">
            <thead className="bg-hone-50 dark:bg-hone-800">
              <tr>
                <th className="px-4 py-2 text-left text-sm font-medium text-hone-600 dark:text-hone-300">Merchant</th>
                <th className="px-4 py-2 text-left text-sm font-medium text-hone-600 dark:text-hone-300">Since</th>
                <th className="px-4 py-2 text-left text-sm font-medium text-hone-600 dark:text-hone-300">Frequency</th>
                <th className="px-4 py-2 text-right text-sm font-medium text-hone-600 dark:text-hone-300">Amount</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-hone-100">
              {activeSubscriptions.map((sub) => (
                <tr key={sub.id} className="hover:bg-hone-50 dark:hover:bg-hone-800">
                  <td className="px-4 py-3 font-medium text-hone-900 dark:text-hone-100">{sub.merchant}</td>
                  <td className="px-4 py-3 text-hone-600">
                    {(() => {
                      const [year, month, day] = sub.first_seen.split("-").map(Number);
                      return new Date(year, month - 1, day, 12, 0, 0).toLocaleDateString("en-US", { month: "short", year: "numeric" });
                    })()}
                  </td>
                  <td className="px-4 py-3 text-hone-600 capitalize">{sub.frequency}</td>
                  <td className="px-4 py-3 text-right font-medium text-hone-900 dark:text-hone-100">
                    ${sub.amount.toFixed(2)}{formatFrequency(sub.frequency)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* No issues state */}
      {zombieSubscriptions.length === 0 && duplicateAlerts.length === 0 && priceIncreaseAlerts.length === 0 && (
        <div className="card p-6 text-center">
          <div className="text-savings text-lg font-semibold">All clear!</div>
          <p className="text-hone-500 mt-1">No subscription issues detected. Your recurring charges look healthy.</p>
        </div>
      )}

      {/* Cancelled subscriptions list */}
      {savingsData && savingsData.cancelled.length > 0 && (
        <div className="card">
          <div className="card-header">
            <h3 className="font-semibold">Cancelled Subscriptions</h3>
          </div>
          <table className="w-full">
            <thead className="bg-hone-50 dark:bg-hone-800">
              <tr>
                <th className="px-4 py-2 text-left text-sm font-medium text-hone-600 dark:text-hone-300">Merchant</th>
                <th className="px-4 py-2 text-right text-sm font-medium text-hone-600 dark:text-hone-300">Was Paying</th>
                <th className="px-4 py-2 text-right text-sm font-medium text-hone-600 dark:text-hone-300">Cancelled</th>
                <th className="px-4 py-2 text-right text-sm font-medium text-hone-600 dark:text-hone-300">Months Saved</th>
                <th className="px-4 py-2 text-right text-sm font-medium text-hone-600 dark:text-hone-300">Total Saved</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-hone-100">
              {savingsData.cancelled.map((sub) => (
                <tr key={sub.id} className="hover:bg-hone-50 dark:hover:bg-hone-800">
                  <td className="px-4 py-3 font-medium text-hone-900 dark:text-hone-100">{sub.merchant}</td>
                  <td className="px-4 py-3 text-right text-hone-600">
                    ${sub.monthly_amount.toLocaleString(undefined, { minimumFractionDigits: 2 })}/mo
                  </td>
                  <td className="px-4 py-3 text-right text-hone-500">{sub.cancelled_at}</td>
                  <td className="px-4 py-3 text-right text-hone-600">{sub.months_counted}</td>
                  <td className="px-4 py-3 text-right font-medium text-savings">
                    ${sub.savings.toLocaleString(undefined, { minimumFractionDigits: 2 })}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
