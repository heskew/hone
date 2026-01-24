import {
  AlertTriangle,
  BadgeDollarSign,
  Menu,
  RefreshCw,
  User,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { api, type MeResponse } from "./api";
import type {
  Account,
  Alert,
  CategorySpending,
  DashboardStats,
  Subscription,
  Transaction,
} from "./types";
import { useHashRouter } from "./hooks";

// Import extracted components
import { Dashboard } from "./components/Dashboard";
import { TransactionsList } from "./components/Transactions";
import { SubscriptionsList } from "./components/Subscriptions";
import { AlertsList } from "./components/Alerts";
import { ImportView, ImportHistoryPage } from "./components/Import";
import { TagsPage } from "./components/Tags";
import { ReportsPage } from "./components/Reports";
import { ReceiptsPage } from "./components/Receipts";
import { OllamaPage } from "./components/Ollama";
import { FeedbackPage } from "./components/Feedback";
import { ExplorePage } from "./components/Explore";
import { Footer } from "./components/common";

export default function App() {
  const { state: routerState, navigate } = useHashRouter();
  const view = routerState.view;

  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [alerts, setAlerts] = useState<Alert[]>([]);
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [currentUser, setCurrentUser] = useState<MeResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  // Dashboard-specific data
  const [recentTransactions, setRecentTransactions] = useState<Transaction[]>([]);
  const [currentMonthSpending, setCurrentMonthSpending] = useState(0);
  const [lastMonthSpending, setLastMonthSpending] = useState(0);
  const [topCategories, setTopCategories] = useState<CategorySpending[]>([]);

  const loadData = async () => {
    try {
      setLoading(true);
      setError(null);

      const [dashboardData, alertsData, subsData, accountsData, meData, transactionsData, currentSpendingData, lastSpendingData] = await Promise.all([
        api.getDashboard(),
        api.getAlerts(),
        api.getSubscriptions(),
        api.getAccounts(),
        api.getMe(),
        api.getTransactions({ limit: 10, sort: "date", order: "desc" }),
        api.getSpendingReport({ period: "this-month" }),
        api.getSpendingReport({ period: "last-month" }),
      ]);

      setStats(dashboardData);
      setAlerts(alertsData);
      setSubscriptions(subsData);
      setAccounts(accountsData);
      setCurrentUser(meData);
      setRecentTransactions(transactionsData.transactions);
      setCurrentMonthSpending(currentSpendingData.total);
      setLastMonthSpending(lastSpendingData.total);
      setTopCategories(currentSpendingData.categories);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load data");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleDismissAlert = async (id: number) => {
    try {
      await api.dismissAlert(id);
      setAlerts(alerts.filter((a) => a.id !== id));
      // Refresh dashboard stats
      const newStats = await api.getDashboard();
      setStats(newStats);
    } catch (err) {
      console.error("Failed to dismiss alert:", err);
    }
  };

  const handleAcknowledgeSubscription = async (id: number) => {
    try {
      await api.acknowledgeSubscription(id);
      setSubscriptions(
        subscriptions.map((s) => s.id === id ? { ...s, user_acknowledged: true, status: "active" as const } : s),
      );
      // Dismiss related alerts
      const relatedAlerts = alerts.filter((a) => a.subscription_id === id);
      for (const alert of relatedAlerts) {
        await api.dismissAlert(alert.id);
      }
      setAlerts(alerts.filter((a) => a.subscription_id !== id));
      // Refresh dashboard
      const newStats = await api.getDashboard();
      setStats(newStats);
    } catch (err) {
      console.error("Failed to acknowledge subscription:", err);
    }
  };

  const handleExcludeSubscription = async (id: number) => {
    // Update local state after AlertDetailModal handles the API call
    setSubscriptions(
      subscriptions.map((s) => s.id === id ? { ...s, status: "excluded" as const } : s),
    );
    setAlerts(alerts.filter((a) => a.subscription_id !== id));
    // Refresh dashboard
    try {
      const newStats = await api.getDashboard();
      setStats(newStats);
    } catch (err) {
      console.error("Failed to refresh dashboard:", err);
    }
  };

  const handleUnexcludeSubscription = async (id: number) => {
    try {
      await api.unexcludeSubscription(id);
      setSubscriptions(
        subscriptions.map((s) => s.id === id ? { ...s, status: "active" as const } : s),
      );
      // Refresh dashboard
      const newStats = await api.getDashboard();
      setStats(newStats);
    } catch (err) {
      console.error("Failed to unexclude subscription:", err);
    }
  };

  const handleCancelSubscription = async (id: number) => {
    // Update local state - AlertDetailModal already called the API
    setSubscriptions(
      subscriptions.map((s) => s.id === id ? { ...s, status: "cancelled" as const } : s),
    );
    // Refresh dashboard to update savings
    try {
      const newStats = await api.getDashboard();
      setStats(newStats);
    } catch (err) {
      console.error("Failed to refresh dashboard:", err);
    }
  };

  if (loading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-hone-50 dark:bg-hone-900">
        <div className="flex items-center gap-3 text-hone-500 dark:text-hone-400">
          <RefreshCw className="w-5 h-5 animate-spin" />
          <span>Loading...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-hone-50 dark:bg-hone-900">
        <div className="card p-8 text-center max-w-md">
          <AlertTriangle className="w-12 h-12 text-attention mx-auto mb-4" />
          <h2 className="text-lg font-semibold text-hone-900 dark:text-hone-100 mb-2">Connection Error</h2>
          <p className="text-hone-600 dark:text-hone-400 mb-4">{error}</p>
          <p className="text-sm text-hone-500 dark:text-hone-500 mb-4">
            Make sure the Hone server is running.
          </p>
          <button onClick={loadData} className="btn-primary">
            <RefreshCw className="w-4 h-4 mr-2" />
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-hone-50 dark:bg-hone-900 text-hone-900 dark:text-hone-50 flex flex-col">
      {/* Header */}
      <header className="bg-white dark:bg-hone-900 border-b border-hone-100 dark:border-hone-700 sticky top-0 z-10">
        <div className="max-w-[1800px] mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex items-center justify-between h-16">
            <div className="flex items-center gap-2">
              <BadgeDollarSign className="w-7 h-7 text-hone-700 dark:text-hone-300" />
              <span className="text-xl font-bold text-hone-900 dark:text-hone-50">Hone</span>
            </div>

            {/* Desktop nav */}
            <nav className="hidden md:flex items-center gap-1">
              {(["dashboard", "transactions", "subscriptions", "alerts", "reports", "tags", "receipts", "import", "history", "feedback", "explore"] as const).map((v) => (
                <button
                  key={v}
                  onClick={() => navigate(v)}
                  className={`px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
                    view === v
                      ? "bg-hone-100 dark:bg-hone-700 text-hone-900 dark:text-hone-50"
                      : "text-hone-500 dark:text-hone-400 hover:text-hone-700 dark:hover:text-hone-200 hover:bg-hone-50 dark:hover:bg-hone-800"
                  }`}
                >
                  {v.charAt(0).toUpperCase() + v.slice(1)}
                  {v === "alerts" && alerts.length > 0 && (
                    <span className="ml-1.5 px-1.5 py-0.5 text-xs bg-waste text-white rounded-full">
                      {alerts.length}
                    </span>
                  )}
                </button>
              ))}
            </nav>

            <div className="flex items-center gap-2">
              {/* User display */}
              {currentUser && currentUser.user !== "local-dev" && (
                <div className="hidden sm:flex items-center gap-1.5 text-sm text-hone-500 dark:text-hone-400">
                  <User className="w-4 h-4" />
                  <span className="max-w-[200px] truncate" title={currentUser.user}>
                    {currentUser.user}
                  </span>
                </div>
              )}

              <button
                onClick={loadData}
                className="btn-ghost"
                title="Refresh data"
              >
                <RefreshCw className="w-4 h-4" />
              </button>

              {/* Mobile menu button */}
              <button
                onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
                className="md:hidden btn-ghost"
                aria-label="Toggle menu"
              >
                {mobileMenuOpen ? <X className="w-5 h-5" /> : <Menu className="w-5 h-5" />}
              </button>
            </div>
          </div>

          {/* Mobile nav */}
          {mobileMenuOpen && (
            <nav className="md:hidden py-3 border-t border-hone-100 dark:border-hone-700">
              <div className="flex flex-col gap-1">
                {(["dashboard", "transactions", "subscriptions", "alerts", "reports", "tags", "receipts", "import", "history", "feedback", "explore"] as const).map((v) => (
                  <button
                    key={v}
                    onClick={() => {
                      navigate(v);
                      setMobileMenuOpen(false);
                    }}
                    className={`px-3 py-2 rounded-lg text-sm font-medium transition-colors text-left ${
                      view === v
                        ? "bg-hone-100 dark:bg-hone-700 text-hone-900 dark:text-hone-50"
                        : "text-hone-500 dark:text-hone-400 hover:text-hone-700 dark:hover:text-hone-200 hover:bg-hone-50 dark:hover:bg-hone-800"
                    }`}
                  >
                    {v.charAt(0).toUpperCase() + v.slice(1)}
                    {v === "alerts" && alerts.length > 0 && (
                      <span className="ml-2 px-1.5 py-0.5 text-xs bg-waste text-white rounded-full">
                        {alerts.length}
                      </span>
                    )}
                  </button>
                ))}
              </div>
            </nav>
          )}
        </div>
      </header>

      {/* Main Content */}
      <main className="max-w-[1800px] w-full mx-auto px-4 sm:px-6 lg:px-8 py-8 flex-1">
        {view === "dashboard" && stats && (
          <Dashboard
            stats={stats}
            alerts={alerts}
            recentTransactions={recentTransactions}
            currentMonthSpending={currentMonthSpending}
            lastMonthSpending={lastMonthSpending}
            topCategories={topCategories}
            subscriptions={subscriptions}
            onDismissAlert={handleDismissAlert}
            onViewAlerts={() => navigate("alerts")}
            onNavigate={navigate}
          />
        )}

        {view === "transactions" && <TransactionsList initialUntagged={routerState.params.untagged === "true"} />}

        {view === "subscriptions" && (
          <SubscriptionsList
            subscriptions={subscriptions}
            accounts={accounts}
            alerts={alerts}
            onAcknowledge={handleAcknowledgeSubscription}
            onCancel={handleCancelSubscription}
            onExclude={handleExcludeSubscription}
            onUnexclude={handleUnexcludeSubscription}
          />
        )}

        {view === "alerts" && (
          <AlertsList
            alerts={alerts}
            onDismiss={handleDismissAlert}
            onAcknowledge={handleAcknowledgeSubscription}
            onRestore={loadData}
            onExclude={handleExcludeSubscription}
            onCancel={handleCancelSubscription}
          />
        )}

        {view === "tags" && <TagsPage />}

        {view === "reports" && <ReportsPage />}

        {view === "receipts" && <ReceiptsPage />}

        {view === "ai-metrics" && <OllamaPage />}

        {view === "import" && (
          <ImportView
            accounts={accounts}
            onAccountCreated={(account) => setAccounts([...accounts, account])}
            onImportComplete={() => {
              loadData();
              navigate("dashboard");
            }}
            onAccountUpdated={() => api.getAccounts().then(setAccounts)}
          />
        )}

        {view === "history" && <ImportHistoryPage accounts={accounts} />}

        {view === "feedback" && <FeedbackPage />}

        {view === "explore" && <ExplorePage />}
      </main>

      {/* Footer */}
      <Footer onNavigateToAIMetrics={() => navigate("ai-metrics")} />
    </div>
  );
}
