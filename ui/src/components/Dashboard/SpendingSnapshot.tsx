import { TrendingDown, TrendingUp, Minus, ArrowRight } from "lucide-react";
import type { View } from "../../hooks";

interface SpendingSnapshotProps {
  currentMonthSpending: number;
  lastMonthSpending: number;
  onNavigate: (view: View, subview?: string | null, params?: Record<string, string>) => void;
}

export function SpendingSnapshot({
  currentMonthSpending,
  lastMonthSpending,
  onNavigate,
}: SpendingSnapshotProps) {
  const change = lastMonthSpending > 0
    ? ((currentMonthSpending - lastMonthSpending) / lastMonthSpending) * 100
    : 0;
  const isUp = change > 5;
  const isDown = change < -5;

  const TrendIcon = isUp ? TrendingUp : isDown ? TrendingDown : Minus;
  const trendColor = isUp
    ? "text-attention"
    : isDown
    ? "text-savings"
    : "text-hone-500";

  // Calculate progress through the month
  const today = new Date();
  const daysInMonth = new Date(today.getFullYear(), today.getMonth() + 1, 0).getDate();
  const dayOfMonth = today.getDate();
  const monthProgress = dayOfMonth / daysInMonth;

  // Projected spending for the full month
  const projectedSpending = monthProgress > 0 ? currentMonthSpending / monthProgress : currentMonthSpending;

  return (
    <div className="card">
      <div className="card-header flex items-center justify-between">
        <h2 className="text-lg font-semibold">This Month</h2>
        <button
          onClick={() => onNavigate("reports", "spending")}
          className="btn-ghost text-sm"
        >
          Details
          <ArrowRight className="w-4 h-4 ml-1" />
        </button>
      </div>
      <div className="card-body">
        <div className="flex items-baseline gap-3 mb-4">
          <span className="text-3xl font-bold text-hone-900 dark:text-hone-100">
            ${currentMonthSpending.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
          </span>
          <div className={`flex items-center gap-1 text-sm font-medium ${trendColor}`}>
            <TrendIcon className="w-4 h-4" />
            {Math.abs(change).toFixed(0)}%
          </div>
        </div>

        {/* Comparison bar */}
        <div className="space-y-2">
          <div className="flex justify-between text-sm text-hone-600 dark:text-hone-400">
            <span>vs last month</span>
            <span>${lastMonthSpending.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}</span>
          </div>
          <div className="h-2 bg-hone-100 dark:bg-hone-700 rounded-full overflow-hidden">
            <div
              className={`h-full rounded-full transition-all duration-500 ${
                isUp ? "bg-attention" : isDown ? "bg-savings" : "bg-hone-400"
              }`}
              style={{
                width: `${Math.min((currentMonthSpending / Math.max(lastMonthSpending, 1)) * 100, 150)}%`,
                maxWidth: "100%",
              }}
            />
          </div>
        </div>

        {/* Projected spending */}
        {monthProgress < 0.9 && projectedSpending > currentMonthSpending * 1.1 && (
          <div className="mt-4 pt-4 border-t border-hone-100 dark:border-hone-700">
            <div className="text-sm text-hone-600 dark:text-hone-400">
              On track for{" "}
              <span className="font-medium text-hone-900 dark:text-hone-100">
                ${projectedSpending.toLocaleString(undefined, { minimumFractionDigits: 0, maximumFractionDigits: 0 })}
              </span>{" "}
              this month
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
