import { useState, useEffect } from "react";
import { RefreshCw } from "lucide-react";
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
import type { MerchantSummary } from "../../types";

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

export function MerchantsTab({ periodParams, filterParams }: { periodParams: PeriodParams; filterParams: FilterParams }) {
  const [data, setData] = useState<MerchantSummary[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const loadData = async () => {
      try {
        setLoading(true);
        setError(null);
        const result = await api.getMerchantsReport({ ...periodParams, ...filterParams, limit: 20 });
        setData(result.merchants);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load merchants data");
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

  if (!data || data.length === 0) {
    return (
      <div className="card p-6 text-center">
        <p className="text-hone-500">No merchant data for this period</p>
      </div>
    );
  }

  // Prepare chart data - top 10 for the chart
  const chartData = data.slice(0, 10).map((m) => ({
    name: m.merchant.length > 18 ? m.merchant.slice(0, 18) + "..." : m.merchant,
    fullName: m.merchant,
    amount: Math.abs(m.amount),
  }));

  const totalSpending = data.reduce((sum, m) => sum + Math.abs(m.amount), 0);

  return (
    <div className="space-y-6">
      {/* Chart */}
      <div className="card p-6">
        <h3 className="text-lg font-semibold text-hone-900 dark:text-hone-100 mb-4">Top 10 Merchants</h3>
        <div className="h-80">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={chartData} layout="vertical" margin={{ left: 100, right: 20 }}>
              <XAxis type="number" tickFormatter={(v) => `$${v.toLocaleString()}`} tick={{ fill: "var(--color-hone-400)" }} />
              <YAxis
                type="category"
                dataKey="name"
                width={100}
                tick={(props) => {
                  const { x, y, payload } = props;
                  const fullName = chartData.find((d) => d.name === payload.value)?.fullName || payload.value;
                  return (
                    <text
                      x={x}
                      y={y}
                      dy={4}
                      textAnchor="end"
                      fill="var(--color-hone-300)"
                      fontSize={12}
                    >
                      <title>{fullName}</title>
                      {payload.value}
                    </text>
                  );
                }}
              />
              <Tooltip
                formatter={(value: number) => [`$${value.toLocaleString(undefined, { minimumFractionDigits: 2 })}`, "Amount"]}
                labelFormatter={(label) => chartData.find((d) => d.name === label)?.fullName || label}
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

      {/* Full table */}
      <div className="card">
        <div className="card-header">
          <h3 className="font-semibold">All Merchants ({data.length})</h3>
        </div>
        <table className="w-full">
          <thead className="bg-hone-50 dark:bg-hone-800">
            <tr>
              <th className="px-4 py-2 text-left text-sm font-medium text-hone-600 dark:text-hone-300">#</th>
              <th className="px-4 py-2 text-left text-sm font-medium text-hone-600 dark:text-hone-300">Merchant</th>
              <th className="px-4 py-2 text-right text-sm font-medium text-hone-600 dark:text-hone-300">Transactions</th>
              <th className="px-4 py-2 text-right text-sm font-medium text-hone-600 dark:text-hone-300">% of Total</th>
              <th className="px-4 py-2 text-right text-sm font-medium text-hone-600 dark:text-hone-300">Amount</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-hone-100">
            {data.map((merchant, index) => {
              const percentage = (Math.abs(merchant.amount) / totalSpending) * 100;
              return (
                <tr key={merchant.merchant} className="hover:bg-hone-50 dark:hover:bg-hone-800">
                  <td className="px-4 py-3 text-hone-400">{index + 1}</td>
                  <td className="px-4 py-3 font-medium text-hone-900 dark:text-hone-100">{merchant.merchant}</td>
                  <td className="px-4 py-3 text-right text-hone-600">{merchant.transaction_count}</td>
                  <td className="px-4 py-3 text-right text-hone-500">{percentage.toFixed(1)}%</td>
                  <td className="px-4 py-3 text-right font-medium text-hone-900 dark:text-hone-100">
                    ${Math.abs(merchant.amount).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
