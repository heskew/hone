import { useState, useEffect } from "react";
import { RefreshCw } from "lucide-react";
import {
  LineChart as RechartsLineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import { api } from "../../api";
import type { TrendsReport, Granularity } from "../../types";

interface PeriodParams {
  period?: string;
  from?: string;
  to?: string;
}

interface FilterParams {
  entity_id?: number;
  card_member?: string;
}

export function TrendsTab({ periodParams, filterParams }: { periodParams: PeriodParams; filterParams: FilterParams }) {
  const [data, setData] = useState<TrendsReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [granularity, setGranularity] = useState<Granularity>("monthly");

  useEffect(() => {
    const loadData = async () => {
      try {
        setLoading(true);
        setError(null);
        const result = await api.getTrendsReport({ ...periodParams, ...filterParams, granularity });
        setData(result);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load trends data");
      } finally {
        setLoading(false);
      }
    };
    loadData();
  }, [periodParams.period, periodParams.from, periodParams.to, filterParams.entity_id, filterParams.card_member, granularity]);

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

  if (!data || data.data.length === 0) {
    return (
      <div className="card p-6 text-center">
        <p className="text-hone-500">No trend data for this period</p>
      </div>
    );
  }

  // Calculate totals
  const totalSpending = data.data.reduce((sum, d) => sum + Math.abs(d.amount), 0);
  const avgSpending = totalSpending / data.data.length;

  // Prepare chart data
  const chartData = data.data.map((d) => ({
    period: d.period,
    amount: Math.abs(d.amount),
    transactions: d.transaction_count,
  }));

  return (
    <div className="space-y-6">
      {/* Granularity toggle and summary */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <div className="card p-4">
            <div className="text-sm text-hone-500">Total</div>
            <div className="text-xl font-bold text-hone-900 dark:text-hone-100">
              ${totalSpending.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
            </div>
          </div>
          <div className="card p-4">
            <div className="text-sm text-hone-500">Avg per {granularity === "monthly" ? "Month" : "Week"}</div>
            <div className="text-xl font-bold text-hone-900 dark:text-hone-100">
              ${avgSpending.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
            </div>
          </div>
        </div>
        <div className="flex gap-1 bg-hone-100 dark:bg-hone-800 p-1 rounded-lg">
          <button
            onClick={() => setGranularity("weekly")}
            className={`px-3 py-1 text-sm rounded ${
              granularity === "weekly" ? "bg-white dark:bg-hone-700 shadow text-hone-900 dark:text-hone-100" : "text-hone-500 dark:text-hone-400"
            }`}
          >
            Weekly
          </button>
          <button
            onClick={() => setGranularity("monthly")}
            className={`px-3 py-1 text-sm rounded ${
              granularity === "monthly" ? "bg-white dark:bg-hone-700 shadow text-hone-900 dark:text-hone-100" : "text-hone-500 dark:text-hone-400"
            }`}
          >
            Monthly
          </button>
        </div>
      </div>

      {/* Chart */}
      <div className="card p-6">
        <h3 className="text-lg font-semibold text-hone-900 dark:text-hone-100 mb-4">Spending Over Time</h3>
        <div className="h-80">
          <ResponsiveContainer width="100%" height="100%">
            <RechartsLineChart data={chartData} margin={{ left: 20, right: 20 }}>
              <XAxis dataKey="period" tick={{ fill: "var(--color-hone-300)" }} />
              <YAxis tickFormatter={(v) => `$${v.toLocaleString()}`} tick={{ fill: "var(--color-hone-400)" }} />
              <Tooltip
                formatter={(value: number) => [`$${value.toLocaleString(undefined, { minimumFractionDigits: 2 })}`, "Spending"]}
                contentStyle={{
                  backgroundColor: "var(--color-hone-800)",
                  border: "1px solid var(--color-hone-700)",
                  borderRadius: "0.5rem",
                }}
                labelStyle={{ color: "var(--color-hone-300)" }}
                itemStyle={{ color: "var(--color-hone-100)" }}
              />
              <Line
                type="monotone"
                dataKey="amount"
                stroke="#0ea5e9"
                strokeWidth={2}
                dot={{ fill: "#0ea5e9", strokeWidth: 2 }}
              />
            </RechartsLineChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* Data table */}
      <div className="card">
        <div className="card-header">
          <h3 className="font-semibold">Period Breakdown</h3>
        </div>
        <table className="w-full">
          <thead className="bg-hone-50 dark:bg-hone-800">
            <tr>
              <th className="px-4 py-2 text-left text-sm font-medium text-hone-600 dark:text-hone-300">Period</th>
              <th className="px-4 py-2 text-right text-sm font-medium text-hone-600 dark:text-hone-300">Transactions</th>
              <th className="px-4 py-2 text-right text-sm font-medium text-hone-600 dark:text-hone-300">Amount</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-hone-100">
            {data.data.map((row) => (
              <tr key={row.period} className="hover:bg-hone-50 dark:hover:bg-hone-800">
                <td className="px-4 py-3 font-medium text-hone-900 dark:text-hone-100">{row.period}</td>
                <td className="px-4 py-3 text-right text-hone-600 dark:text-hone-400">{row.transaction_count}</td>
                <td className="px-4 py-3 text-right font-medium text-hone-900 dark:text-hone-100">
                  ${Math.abs(row.amount).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
