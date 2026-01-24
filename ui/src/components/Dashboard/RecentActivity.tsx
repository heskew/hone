import { ArrowRight, Clock } from "lucide-react";
import type { Transaction } from "../../types";
import type { View } from "../../hooks";

interface RecentActivityProps {
  transactions: Transaction[];
  onNavigate: (view: View, subview?: string | null, params?: Record<string, string>) => void;
}

export function RecentActivity({ transactions, onNavigate }: RecentActivityProps) {
  if (transactions.length === 0) {
    return (
      <div className="card">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold flex items-center gap-2">
            <Clock className="w-5 h-5 text-hone-500" />
            Recent Activity
          </h2>
          <button
            onClick={() => onNavigate("transactions")}
            className="btn-ghost text-sm"
          >
            View all
            <ArrowRight className="w-4 h-4 ml-1" />
          </button>
        </div>
        <div className="card-body text-center py-8">
          <p className="text-hone-500">No transactions yet</p>
        </div>
      </div>
    );
  }

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    const today = new Date();
    const yesterday = new Date(today);
    yesterday.setDate(yesterday.getDate() - 1);

    if (date.toDateString() === today.toDateString()) {
      return "Today";
    } else if (date.toDateString() === yesterday.toDateString()) {
      return "Yesterday";
    } else {
      return date.toLocaleDateString("en-US", { month: "short", day: "numeric" });
    }
  };

  return (
    <div className="card">
      <div className="card-header flex items-center justify-between">
        <h2 className="text-lg font-semibold flex items-center gap-2">
          <Clock className="w-5 h-5 text-hone-500" />
          Recent Activity
        </h2>
        <button
          onClick={() => onNavigate("transactions")}
          className="btn-ghost text-sm"
        >
          View all
          <ArrowRight className="w-4 h-4 ml-1" />
        </button>
      </div>
      <div className="divide-y divide-hone-100 dark:divide-hone-700">
        {transactions.map((tx) => (
          <div
            key={tx.id}
            className="px-4 py-3 flex items-center justify-between hover:bg-hone-50 dark:hover:bg-hone-800 cursor-pointer transition-colors"
            onClick={() => onNavigate("transactions", tx.id.toString())}
          >
            <div className="flex-1 min-w-0">
              <div className="font-medium text-hone-900 dark:text-hone-100 truncate">
                {tx.merchant_normalized || tx.description}
              </div>
              <div className="text-sm text-hone-500 flex items-center gap-2">
                <span>{formatDate(tx.date)}</span>
                {tx.tags && tx.tags.length > 0 && (
                  <>
                    <span className="text-hone-300">·</span>
                    <span className="truncate">{tx.tags[0].tag_name}</span>
                  </>
                )}
              </div>
            </div>
            <div className={`font-mono font-medium ${tx.amount < 0 ? "text-savings" : "text-hone-900 dark:text-hone-100"}`}>
              {tx.amount < 0 ? "+" : "-"}${Math.abs(tx.amount).toFixed(2)}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
