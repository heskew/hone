import { useState, useEffect } from "react";
import { RefreshCw, X, ChevronRight } from "lucide-react";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  Cell,
} from "recharts";
import { api } from "../../api";
import type { SpendingSummary, CategorySpending, Transaction } from "../../types";
import { SplitsModal } from "../Transactions/SplitsModal";

interface PeriodParams {
  period?: string;
  from?: string;
  to?: string;
}

interface FilterParams {
  entity_id?: number;
  card_member?: string;
}

const COLORS = [
  "#0ea5e9", "#8b5cf6", "#f59e0b", "#10b981", "#ef4444",
  "#6366f1", "#ec4899", "#14b8a6", "#f97316", "#84cc16",
];

interface SpendingCategoryRowProps {
  category: CategorySpending;
  depth: number;
  onViewTransactions: (category: CategorySpending) => void;
}

function SpendingCategoryRow({ category, depth, onViewTransactions }: SpendingCategoryRowProps) {
  const [expanded, setExpanded] = useState(false);
  const hasChildren = category.children && category.children.length > 0;

  const handleClick = () => {
    if (hasChildren) {
      setExpanded(!expanded);
    } else {
      onViewTransactions(category);
    }
  };

  return (
    <>
      <div
        className="flex items-center justify-between px-4 py-3 hover:bg-hone-50 dark:hover:bg-hone-800 cursor-pointer"
        style={{ paddingLeft: `${16 + depth * 20}px` }}
        onClick={handleClick}
      >
        <div className="flex items-center gap-2">
          {hasChildren && (
            <span className={`text-hone-400 transition-transform ${expanded ? "rotate-90" : ""}`}>
              ▶
            </span>
          )}
          <span className="font-medium text-hone-900 dark:text-hone-100">{category.tag}</span>
          <span className="text-sm text-hone-400">({category.transaction_count} txn)</span>
        </div>
        <div className="flex items-center gap-4">
          <span className="text-sm text-hone-500">{category.percentage.toFixed(1)}%</span>
          <span className="font-medium text-hone-900 dark:text-hone-100 w-24 text-right">
            ${Math.abs(category.amount).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
          </span>
          <ChevronRight className="w-4 h-4 text-hone-400" />
        </div>
      </div>
      {expanded && hasChildren && (
        <>
          {category.children.map((child) => (
            <SpendingCategoryRow
              key={child.tag_id}
              category={child}
              depth={depth + 1}
              onViewTransactions={onViewTransactions}
            />
          ))}
        </>
      )}
    </>
  );
}

interface CategoryTransactionsModalProps {
  category: CategorySpending;
  periodParams: PeriodParams;
  filterParams: FilterParams;
  onClose: () => void;
}

function CategoryTransactionsModal({ category, periodParams, filterParams, onClose }: CategoryTransactionsModalProps) {
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedTransaction, setSelectedTransaction] = useState<Transaction | null>(null);

  useEffect(() => {
    const loadTransactions = async () => {
      try {
        setLoading(true);
        // For untagged (tag_id = -1), use untagged filter; otherwise use tag_ids
        const params: Record<string, unknown> = {
          limit: 100,
          sort: "date",
          order: "desc",
          ...filterParams,
        };

        // Pass date filtering params - either explicit dates or period preset
        if (periodParams.from) params.from = periodParams.from;
        if (periodParams.to) params.to = periodParams.to;
        if (periodParams.period) params.period = periodParams.period;

        if (category.tag_id === -1) {
          // Untagged transactions
          params.untagged = true;
        } else {
          params.tag_ids = [category.tag_id];
        }

        const result = await api.getTransactions(params as Parameters<typeof api.getTransactions>[0]);
        setTransactions(result.transactions);
      } catch (err) {
        console.error("Failed to load transactions:", err);
      } finally {
        setLoading(false);
      }
    };
    loadTransactions();
  }, [category.tag_id, periodParams, filterParams]);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !selectedTransaction) {
        onClose();
      }
    };
    document.addEventListener("keydown", handleEscape);
    return () => document.removeEventListener("keydown", handleEscape);
  }, [onClose, selectedTransaction]);

  const formatDate = (dateStr: string) => {
    // Parse as local date to avoid timezone shift
    const [year, month, day] = dateStr.split("-").map(Number);
    return new Date(year, month - 1, day, 12, 0, 0).toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "2-digit",
    });
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={onClose}>
      <div
        className="card w-full max-w-3xl mx-4 max-h-[85vh] flex flex-col animate-slide-up"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="card-header flex items-center justify-between">
          <div>
            <h2 className="text-lg font-semibold text-hone-900 dark:text-hone-100">
              {category.tag} Transactions
            </h2>
            <p className="text-sm text-hone-500">
              {loading ? "Loading..." : `${transactions.length} transactions · $${Math.abs(transactions.reduce((sum, t) => sum + t.amount, 0)).toLocaleString(undefined, { minimumFractionDigits: 2 })}`}
            </p>
          </div>
          <button onClick={onClose} className="p-1 text-hone-400 hover:text-hone-600">
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="card-body overflow-auto flex-1">
          {loading ? (
            <div className="flex items-center justify-center py-8">
              <RefreshCw className="w-5 h-5 animate-spin text-hone-400" />
            </div>
          ) : transactions.length === 0 ? (
            <p className="text-center py-8 text-hone-500">No transactions found</p>
          ) : (
            <div className="divide-y divide-hone-100 dark:divide-hone-700">
              {transactions.map((tx) => {
                const isExpense = tx.amount < 0;
                return (
                  <div
                    key={tx.id}
                    onClick={() => setSelectedTransaction(tx)}
                    className="flex items-center justify-between px-4 py-3 hover:bg-hone-50 dark:hover:bg-hone-800 cursor-pointer"
                  >
                    <div className="flex-1 min-w-0">
                      <p className="font-medium text-hone-900 dark:text-hone-100 truncate">
                        {tx.merchant_normalized || tx.description}
                      </p>
                      <p className="text-sm text-hone-500">{formatDate(tx.date)}</p>
                    </div>
                    <div className={`font-semibold ml-4 ${isExpense ? "text-hone-700 dark:text-hone-300" : "text-savings"}`}>
                      {isExpense ? "-" : "+"}${Math.abs(tx.amount).toFixed(2)}
                    </div>
                  </div>
                );
              })}
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

export function SpendingTab({ periodParams, filterParams }: { periodParams: PeriodParams; filterParams: FilterParams }) {
  const [data, setData] = useState<SpendingSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedCategory, setSelectedCategory] = useState<CategorySpending | null>(null);

  useEffect(() => {
    const loadData = async () => {
      try {
        setLoading(true);
        setError(null);
        const result = await api.getSpendingReport({ ...periodParams, ...filterParams, expand: true });
        setData(result);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load spending data");
      } finally {
        setLoading(false);
      }
    };
    loadData();
  }, [periodParams.period, periodParams.from, periodParams.to, filterParams.entity_id, filterParams.card_member]);

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

  if (!data || (data.categories.length === 0 && data.untagged.transaction_count === 0)) {
    return (
      <div className="card p-6 text-center">
        <p className="text-hone-500">No spending data for this period</p>
      </div>
    );
  }

  // Include untagged as a pseudo-category for display
  const allCategories = [...data.categories];
  if (data.untagged.transaction_count > 0) {
    allCategories.push({
      tag: "Untagged",
      tag_id: -1,
      amount: data.untagged.amount,
      percentage: data.untagged.percentage,
      transaction_count: data.untagged.transaction_count,
      children: [],
    });
  }

  // Prepare chart data - top categories by amount
  const chartData = [...allCategories]
    .sort((a, b) => b.amount - a.amount)
    .slice(0, 10)
    .map((cat) => ({
      name: cat.tag,
      amount: Math.abs(cat.amount),
      percentage: cat.percentage,
    }));

  return (
    <div className="space-y-6">
      {/* Summary cards */}
      <div className="grid grid-cols-3 gap-4">
        <div className="card p-4">
          <div className="text-sm text-hone-500">Total Spending</div>
          <div className="text-2xl font-bold text-hone-900 dark:text-hone-100">
            ${Math.abs(data.total).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
          </div>
        </div>
        <div className="card p-4">
          <div className="text-sm text-hone-500">Categories</div>
          <div className="text-2xl font-bold text-hone-900 dark:text-hone-100">{data.categories.length}</div>
        </div>
        <div className="card p-4">
          <div className="text-sm text-hone-500">Untagged</div>
          <div className="text-2xl font-bold text-attention">
            ${Math.abs(data.untagged.amount).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
            <span className="text-sm font-normal text-hone-400 ml-1">
              ({data.untagged.percentage.toFixed(1)}%)
            </span>
          </div>
        </div>
      </div>

      {/* Chart */}
      <div className="card p-6">
        <h3 className="text-lg font-semibold text-hone-900 dark:text-hone-100 mb-4">Spending by Category</h3>
        <div className="h-80">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={chartData} layout="vertical" margin={{ left: 80, right: 20 }}>
              <XAxis type="number" tickFormatter={(v) => `$${v.toLocaleString()}`} tick={{ fill: "var(--color-hone-400)" }} />
              <YAxis type="category" dataKey="name" width={80} tick={{ fill: "var(--color-hone-300)" }} />
              <Tooltip
                formatter={(value: number) => [`$${value.toLocaleString(undefined, { minimumFractionDigits: 2 })}`, "Amount"]}
                contentStyle={{
                  backgroundColor: "var(--color-hone-800)",
                  border: "1px solid var(--color-hone-700)",
                  borderRadius: "0.5rem",
                }}
                labelStyle={{ color: "var(--color-hone-300)" }}
                itemStyle={{ color: "var(--color-hone-100)" }}
                cursor={false}
              />
              <Bar dataKey="amount" radius={[0, 4, 4, 0]}>
                {chartData.map((_, index) => (
                  <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
                ))}
              </Bar>
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* Table */}
      <div className="card">
        <div className="card-header">
          <h3 className="font-semibold">All Categories</h3>
          <p className="text-sm text-hone-400 mt-1">Click a category to view transactions</p>
        </div>
        <div className="divide-y divide-hone-100">
          {allCategories
            .sort((a, b) => b.amount - a.amount)
            .map((cat) => (
              <SpendingCategoryRow
                key={cat.tag_id}
                category={cat}
                depth={0}
                onViewTransactions={setSelectedCategory}
              />
            ))}
        </div>
      </div>

      {/* Category transactions modal */}
      {selectedCategory && (
        <CategoryTransactionsModal
          category={selectedCategory}
          periodParams={periodParams}
          filterParams={filterParams}
          onClose={() => setSelectedCategory(null)}
        />
      )}
    </div>
  );
}
