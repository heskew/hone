import {
  AlertTriangle,
  ArrowRight,
  BadgeDollarSign,
  CreditCard,
  LineChart,
  Receipt,
  Tag,
  TrendingUp,
} from "lucide-react";
import type { Alert, CategorySpending, DashboardStats, Subscription, Transaction } from "../../types";
import type { View } from "../../hooks";
import { StatCard } from "../common";
import { AlertItem } from "./AlertItem";
import { InsightsWidget } from "../Insights/InsightsWidget";
import { RecentActivity } from "./RecentActivity";
import { SpendingSnapshot } from "./SpendingSnapshot";
import { TopCategories } from "./TopCategories";
import { UpcomingCharges } from "./UpcomingCharges";

interface DashboardProps {
  stats: DashboardStats;
  alerts: Alert[];
  recentTransactions: Transaction[];
  currentMonthSpending: number;
  lastMonthSpending: number;
  topCategories: CategorySpending[];
  subscriptions: Subscription[];
  onDismissAlert: (id: number) => void;
  onViewAlerts: () => void;
  onNavigate: (view: View, subview?: string | null, params?: Record<string, string>) => void;
}

export function Dashboard({
  stats,
  alerts,
  recentTransactions,
  currentMonthSpending,
  lastMonthSpending,
  topCategories,
  subscriptions,
  onDismissAlert,
  onViewAlerts,
  onNavigate,
}: DashboardProps) {
  const hasData = stats.total_transactions > 0;

  return (
    <div className="space-y-8 animate-fade-in">
      <h1 className="sr-only">Dashboard</h1>
      {/* Stats Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-4">
        <StatCard
          icon={<CreditCard className="w-5 h-5" />}
          label="Accounts"
          value={stats.total_accounts.toString()}
          onClick={() => onNavigate("import")}
        />
        <StatCard
          icon={<Receipt className="w-5 h-5" />}
          label="Transactions"
          value={stats.total_transactions.toLocaleString()}
          onClick={() => onNavigate("transactions")}
        />
        <StatCard
          icon={<Tag className="w-5 h-5" />}
          label="Untagged"
          value={stats.untagged_transactions.toLocaleString()}
          attention={stats.untagged_transactions > 0}
          onClick={() => onNavigate("transactions", null, { untagged: "true" })}
        />
        <StatCard
          icon={<LineChart className="w-5 h-5" />}
          label="Active Subscriptions"
          value={stats.active_subscriptions.toString()}
          subtext={`$${stats.monthly_subscription_cost.toFixed(2)}/mo`}
          onClick={() => onNavigate("subscriptions")}
        />
        <StatCard
          icon={<TrendingUp className="w-5 h-5" />}
          label="Potential Savings"
          value={`$${stats.potential_monthly_savings.toFixed(2)}`}
          subtext="per month"
          highlight={stats.potential_monthly_savings > 0}
          onClick={() => onNavigate("alerts")}
        />
      </div>

      {/* Insights Widget - proactive financial insights */}
      {hasData && <InsightsWidget limit={5} />}

      {/* Active Alerts Section */}
      {alerts.length > 0 && (
        <div className="card">
          <div className="card-header flex items-center justify-between">
            <h2 className="text-lg font-semibold flex items-center gap-2">
              <AlertTriangle className="w-5 h-5 text-attention" />
              Needs Your Attention
            </h2>
            <button onClick={onViewAlerts} className="btn-ghost text-sm">
              View all
              <ArrowRight className="w-4 h-4 ml-1" />
            </button>
          </div>
          <div className="divide-y divide-hone-100 dark:divide-hone-700">
            {alerts.slice(0, 3).map((alert) => <AlertItem key={alert.id} alert={alert} onDismiss={onDismissAlert} />)}
          </div>
        </div>
      )}

      {/* Spending and Categories Grid */}
      {hasData && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <SpendingSnapshot
            currentMonthSpending={currentMonthSpending}
            lastMonthSpending={lastMonthSpending}
            onNavigate={onNavigate}
          />
          <TopCategories
            categories={topCategories}
            total={currentMonthSpending}
            onNavigate={onNavigate}
          />
        </div>
      )}

      {/* Recent Activity and Upcoming Charges Grid */}
      {hasData && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <RecentActivity
            transactions={recentTransactions}
            onNavigate={onNavigate}
          />
          <UpcomingCharges
            subscriptions={subscriptions}
            onNavigate={onNavigate}
          />
        </div>
      )}

      {/* Empty State */}
      {!hasData && (
        <div className="card p-12 text-center">
          <BadgeDollarSign className="w-16 h-16 text-hone-300 mx-auto mb-4" />
          <h2 className="text-xl font-semibold mb-2">Welcome to Hone</h2>
          <p className="text-hone-500 mb-6 max-w-md mx-auto">
            Import your bank transactions to get started. Hone will analyze your spending and find opportunities to
            save.
          </p>
          <div className="font-mono text-sm bg-hone-100 dark:bg-hone-800 rounded-lg p-4 max-w-md mx-auto text-left text-hone-900 dark:text-hone-100">
            <p className="text-hone-500 dark:text-hone-400"># Import transactions</p>
            <p>hone import --file statement.csv --bank chase</p>
            <p className="text-hone-500 dark:text-hone-400 mt-2"># Run detection</p>
            <p>hone detect --kind all</p>
          </div>
        </div>
      )}

      {/* All Clear State - only show if we have data but no alerts */}
      {hasData && alerts.length === 0 && (
        <div className="card p-8 text-center">
          <div className="w-16 h-16 bg-savings-light rounded-full flex items-center justify-center mx-auto mb-4">
            <BadgeDollarSign className="w-8 h-8 text-savings" />
          </div>
          <h2 className="text-xl font-semibold mb-2">Looking Good!</h2>
          <p className="text-hone-500">
            No wasteful spending detected. Keep up the good work!
          </p>
        </div>
      )}
    </div>
  );
}
