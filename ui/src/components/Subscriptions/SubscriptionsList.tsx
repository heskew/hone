import { Ban, Ghost, Receipt } from "lucide-react";
import { useState } from "react";
import type { Account, Alert, Subscription } from "../../types";
import { SubscriptionItem } from "./SubscriptionItem";

interface SubscriptionsListProps {
  subscriptions: Subscription[];
  accounts: Account[];
  alerts: Alert[];
  onAcknowledge: (id: number) => void;
  onCancel: (id: number) => void;
  onExclude: (id: number) => void;
  onUnexclude: (id: number) => void;
}

export function SubscriptionsList({
  subscriptions,
  accounts,
  alerts,
  onAcknowledge,
  onCancel,
  onExclude,
  onUnexclude,
}: SubscriptionsListProps) {
  const [accountFilter, setAccountFilter] = useState<number | null>(null);
  const [showExcluded, setShowExcluded] = useState(false);

  // Apply account filter
  const filteredSubscriptions = accountFilter
    ? subscriptions.filter((s) => s.account_id === accountFilter)
    : subscriptions;

  if (subscriptions.length === 0) {
    return (
      <div className="space-y-6 animate-fade-in">
        <h1 className="text-2xl font-bold text-hone-900 dark:text-hone-50">Subscriptions</h1>
        <div className="card p-8 text-center">
          <Receipt className="w-12 h-12 text-hone-300 mx-auto mb-4" />
          <h2 className="text-lg font-semibold mb-2 text-hone-900 dark:text-hone-50">No Subscriptions Detected</h2>
          <p className="text-hone-500 dark:text-hone-400">
            Import transactions and run detection to find recurring charges.
          </p>
        </div>
      </div>
    );
  }

  const active = filteredSubscriptions.filter((s) => s.status === "active");
  const zombies = filteredSubscriptions.filter((s) => s.status === "zombie");
  const excluded = filteredSubscriptions.filter((s) => s.status === "excluded");

  // Helper to get account name
  const getAccountName = (accountId: number | null) => {
    if (!accountId) return null;
    const account = accounts.find((a) => a.id === accountId);
    return account ? account.name : null;
  };

  return (
    <div className="space-y-6 animate-fade-in">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <h1 className="text-2xl font-bold text-hone-900 dark:text-hone-50">Subscriptions</h1>

        {/* Account filter */}
        {accounts.length > 0 && (
          <div className="flex items-center gap-2">
            <label className="text-sm text-hone-500 dark:text-hone-400">Account:</label>
            <select
              value={accountFilter ?? ""}
              onChange={(e) => setAccountFilter(e.target.value ? Number(e.target.value) : null)}
              className="text-sm border border-hone-200 dark:border-hone-600 rounded-lg px-3 py-1.5 bg-white dark:bg-hone-800 text-hone-900 dark:text-hone-100"
            >
              <option value="">All accounts</option>
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.name}
                </option>
              ))}
            </select>
          </div>
        )}
      </div>

      {zombies.length > 0 && (
        <div className="card">
          <div className="card-header">
            <h2 className="text-lg font-semibold flex items-center gap-2">
              <Ghost className="w-5 h-5 text-attention" />
              Possible Zombies
              <span className="badge-warning">{zombies.length}</span>
            </h2>
          </div>
          <div className="divide-y divide-hone-100 dark:divide-hone-800">
            {zombies.map((sub) => (
              <SubscriptionItem
                key={sub.id}
                subscription={sub}
                accountName={getAccountName(sub.account_id)}
                alerts={alerts}
                onAcknowledge={onAcknowledge}
                onCancel={onCancel}
                onExclude={onExclude}
                onUnexclude={onUnexclude}
              />
            ))}
          </div>
        </div>
      )}

      <div className="card">
        <div className="card-header">
          <h2 className="text-lg font-semibold">Active Subscriptions</h2>
        </div>
        <div className="divide-y divide-hone-100 dark:divide-hone-800">
          {active.length === 0 ? (
            <div className="p-4 text-center text-hone-500 dark:text-hone-400">
              No active subscriptions
            </div>
          ) : (
            active.map((sub) => (
              <SubscriptionItem
                key={sub.id}
                subscription={sub}
                accountName={getAccountName(sub.account_id)}
                alerts={alerts}
                onAcknowledge={onAcknowledge}
                onCancel={onCancel}
                onExclude={onExclude}
                onUnexclude={onUnexclude}
              />
            ))
          )}
        </div>
      </div>

      {/* Show excluded toggle */}
      {excluded.length > 0 && (
        <label className="flex items-center gap-2 text-sm text-hone-600 dark:text-hone-400 cursor-pointer">
          <input
            type="checkbox"
            checked={showExcluded}
            onChange={(e) => setShowExcluded(e.target.checked)}
            className="rounded border-hone-300 dark:border-hone-600 text-hone-600 focus:ring-hone-500 dark:bg-hone-700"
          />
          Show excluded ({excluded.length})
        </label>
      )}

      {/* Excluded subscriptions */}
      {showExcluded && excluded.length > 0 && (
        <div className="card opacity-75">
          <div className="card-header">
            <h2 className="text-lg font-semibold flex items-center gap-2 text-hone-500">
              <Ban className="w-5 h-5" />
              Excluded (Not Subscriptions)
            </h2>
          </div>
          <div className="divide-y divide-hone-100 dark:divide-hone-800">
            {excluded.map((sub) => (
              <SubscriptionItem
                key={sub.id}
                subscription={sub}
                accountName={getAccountName(sub.account_id)}
                alerts={alerts}
                onAcknowledge={onAcknowledge}
                onCancel={onCancel}
                onExclude={onExclude}
                onUnexclude={onUnexclude}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
