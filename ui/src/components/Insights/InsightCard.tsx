import { AlertTriangle, Clock, DollarSign, Info, TrendingDown, TrendingUp, X } from "lucide-react";
import { useState } from "react";
import { api } from "../../api";
import type {
  ExpenseForecasterData,
  InsightFinding,
  InsightSeverity,
  SavingsOpportunityData,
  SpendingExplainerData,
} from "../../types";

interface InsightCardProps {
  insight: InsightFinding;
  onDismiss?: () => void;
  onSnooze?: () => void;
  compact?: boolean;
}

const severityStyles: Record<InsightSeverity, { bg: string; border: string; icon: string }> = {
  info: {
    bg: "bg-blue-50 dark:bg-blue-900/20",
    border: "border-blue-200 dark:border-blue-800",
    icon: "text-blue-500",
  },
  attention: {
    bg: "bg-amber-50 dark:bg-amber-900/20",
    border: "border-amber-200 dark:border-amber-800",
    icon: "text-amber-500",
  },
  warning: {
    bg: "bg-orange-50 dark:bg-orange-900/20",
    border: "border-orange-200 dark:border-orange-800",
    icon: "text-orange-500",
  },
  alert: {
    bg: "bg-red-50 dark:bg-red-900/20",
    border: "border-red-200 dark:border-red-800",
    icon: "text-red-500",
  },
};

function SeverityIcon({ severity }: { severity: InsightSeverity }) {
  const className = `w-5 h-5 ${severityStyles[severity].icon}`;

  switch (severity) {
    case "alert":
    case "warning":
      return <AlertTriangle className={className} />;
    case "attention":
      return <Clock className={className} />;
    default:
      return <Info className={className} />;
  }
}

function InsightTypeIcon({ type }: { type: string }) {
  const className = "w-4 h-4";

  switch (type) {
    case "spending_explainer":
      return <TrendingUp className={className} />;
    case "expense_forecaster":
      return <Clock className={className} />;
    case "savings_opportunity":
      return <DollarSign className={className} />;
    default:
      return <Info className={className} />;
  }
}

function SpendingExplainerDetails({ data }: { data: SpendingExplainerData }) {
  const isIncrease = data.percent_change > 0;

  return (
    <div className="mt-3 space-y-2">
      <div className="flex items-center gap-2 text-sm">
        {isIncrease ? (
          <TrendingUp className="w-4 h-4 text-waste" />
        ) : (
          <TrendingDown className="w-4 h-4 text-income" />
        )}
        <span className={isIncrease ? "text-waste" : "text-income"}>
          {isIncrease ? "+" : ""}{data.percent_change.toFixed(0)}%
        </span>
        <span className="text-hone-500">
          ${data.current_amount.toFixed(0)} vs ${data.baseline_amount.toFixed(0)}/mo avg
        </span>
      </div>

      {data.top_merchants.length > 0 && (
        <div className="text-xs text-hone-500">
          <span className="font-medium">Top merchants: </span>
          {data.top_merchants.slice(0, 3).map((m) => m.merchant).join(", ")}
        </div>
      )}

      {data.explanation && (
        <p className="text-sm text-hone-600 dark:text-hone-400 italic">
          {data.explanation}
        </p>
      )}
    </div>
  );
}

function ExpenseForecasterDetails({ data }: { data: ExpenseForecasterData }) {
  const subscriptions = data.items.filter((i) => i.item_type === "subscription");
  const estimates = data.items.filter((i) => i.item_type === "estimate");
  const largeExpenses = data.items.filter((i) => i.item_type === "large_expense");

  return (
    <div className="mt-3 space-y-2">
      <div className="text-sm font-medium text-hone-700 dark:text-hone-300">
        Next {data.period_days} days: ${data.total_expected.toFixed(0)}
      </div>

      {subscriptions.length > 0 && (
        <div className="text-xs">
          <span className="text-hone-500">Subscriptions: </span>
          {subscriptions.slice(0, 3).map((s, i) => (
            <span key={s.name}>
              {i > 0 && ", "}
              {s.name} (${s.amount.toFixed(0)})
            </span>
          ))}
        </div>
      )}

      {estimates.length > 0 && (
        <div className="text-xs text-hone-500">
          <span className="font-medium">Estimates: </span>
          {estimates.map((e) => e.name).join(", ")}
        </div>
      )}

      {largeExpenses.length > 0 && (
        <div className="text-xs text-amber-600 dark:text-amber-400">
          <span className="font-medium">Upcoming: </span>
          {largeExpenses.map((e) => `${e.name} ($${e.amount.toFixed(0)})`).join(", ")}
        </div>
      )}
    </div>
  );
}

function SavingsOpportunityDetails({ data }: { data: SavingsOpportunityData }) {
  return (
    <div className="mt-3 space-y-2">
      <div className="flex items-center gap-2 text-sm">
        <DollarSign className="w-4 h-4 text-income" />
        <span className="text-income font-medium">
          Save ${data.annual_savings.toFixed(0)}/year
        </span>
        <span className="text-hone-500">
          (${data.monthly_amount.toFixed(2)}/mo)
        </span>
      </div>

      {data.subscription_name && (
        <div className="text-xs text-hone-500">
          <span className="font-medium">Service: </span>
          {data.subscription_name}
        </div>
      )}

      <p className="text-xs text-hone-600 dark:text-hone-400">
        {data.reason}
      </p>
    </div>
  );
}

export function InsightCard({ insight, onDismiss, onSnooze, compact = false }: InsightCardProps) {
  const [dismissing, setDismissing] = useState(false);
  const [showSnoozeMenu, setShowSnoozeMenu] = useState(false);
  const styles = severityStyles[insight.severity];

  const handleDismiss = async () => {
    setDismissing(true);
    try {
      await api.dismissInsight(insight.id);
      onDismiss?.();
    } catch (error) {
      console.error("Failed to dismiss insight:", error);
    } finally {
      setDismissing(false);
    }
  };

  const handleSnooze = async (days: number) => {
    setShowSnoozeMenu(false);
    try {
      await api.snoozeInsight(insight.id, days);
      onSnooze?.();
    } catch (error) {
      console.error("Failed to snooze insight:", error);
    }
  };

  return (
    <div className={`rounded-lg border p-4 ${styles.bg} ${styles.border}`}>
      <div className="flex items-start gap-3">
        <SeverityIcon severity={insight.severity} />

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <InsightTypeIcon type={insight.insight_type} />
            <h3 className="font-medium text-hone-900 dark:text-hone-100 truncate">
              {insight.title}
            </h3>
          </div>

          <p className="text-sm text-hone-600 dark:text-hone-400 mt-1">
            {insight.summary}
          </p>

          {!compact && (
            <>
              {insight.insight_type === "spending_explainer" && (
                <SpendingExplainerDetails data={insight.data as SpendingExplainerData} />
              )}
              {insight.insight_type === "expense_forecaster" && (
                <ExpenseForecasterDetails data={insight.data as ExpenseForecasterData} />
              )}
              {insight.insight_type === "savings_opportunity" && (
                <SavingsOpportunityDetails data={insight.data as SavingsOpportunityData} />
              )}

              {insight.detail && (
                <p className="mt-2 text-sm text-hone-500">{insight.detail}</p>
              )}
            </>
          )}
        </div>

        <div className="flex items-center gap-1">
          {!compact && (
            <div className="relative">
              <button
                onClick={() => setShowSnoozeMenu(!showSnoozeMenu)}
                className="p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200 rounded"
                title="Snooze"
              >
                <Clock className="w-4 h-4" />
              </button>

              {showSnoozeMenu && (
                <div className="absolute right-0 top-full mt-1 bg-white dark:bg-hone-800 rounded-lg shadow-lg border border-hone-200 dark:border-hone-700 py-1 z-10">
                  <button
                    onClick={() => handleSnooze(7)}
                    className="block w-full px-3 py-1 text-sm text-left hover:bg-hone-100 dark:hover:bg-hone-700"
                  >
                    7 days
                  </button>
                  <button
                    onClick={() => handleSnooze(14)}
                    className="block w-full px-3 py-1 text-sm text-left hover:bg-hone-100 dark:hover:bg-hone-700"
                  >
                    14 days
                  </button>
                  <button
                    onClick={() => handleSnooze(30)}
                    className="block w-full px-3 py-1 text-sm text-left hover:bg-hone-100 dark:hover:bg-hone-700"
                  >
                    30 days
                  </button>
                </div>
              )}
            </div>
          )}

          <button
            onClick={handleDismiss}
            disabled={dismissing}
            className="p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200 rounded disabled:opacity-50"
            title="Dismiss"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>
  );
}
