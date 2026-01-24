import { ArrowRight, Tag } from "lucide-react";
import type { CategorySpending } from "../../types";
import type { View } from "../../hooks";

interface TopCategoriesProps {
  categories: CategorySpending[];
  total: number;
  onNavigate: (view: View, subview?: string | null, params?: Record<string, string>) => void;
}

// Tag colors from the design system
const TAG_COLORS: Record<string, string> = {
  Transport: "#3B82F6",
  Groceries: "#10B981",
  Dining: "#F59E0B",
  Shopping: "#8B5CF6",
  Entertainment: "#EC4899",
  Subscriptions: "#6366F1",
  Healthcare: "#EF4444",
  Travel: "#14B8A6",
  Utilities: "#64748B",
  Housing: "#78716C",
  Financial: "#0EA5E9",
  Income: "#22C55E",
  Personal: "#A855F7",
  Education: "#06B6D4",
  Pets: "#F97316",
  Gifts: "#E11D48",
  Other: "#9CA3AF",
};

export function TopCategories({ categories, total, onNavigate }: TopCategoriesProps) {
  if (categories.length === 0) {
    return (
      <div className="card">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold flex items-center gap-2">
            <Tag className="w-5 h-5 text-hone-500" />
            Top Categories
          </h2>
          <button
            onClick={() => onNavigate("reports", "spending")}
            className="btn-ghost text-sm"
          >
            See all
            <ArrowRight className="w-4 h-4 ml-1" />
          </button>
        </div>
        <div className="card-body text-center py-8">
          <p className="text-hone-500">No spending data this month</p>
        </div>
      </div>
    );
  }

  // Take top 5 categories
  const topCategories = categories.slice(0, 5);
  const maxAmount = Math.max(...topCategories.map((c) => c.amount));

  return (
    <div className="card">
      <div className="card-header flex items-center justify-between">
        <h2 className="text-lg font-semibold flex items-center gap-2">
          <Tag className="w-5 h-5 text-hone-500" />
          Top Categories
        </h2>
        <button
          onClick={() => onNavigate("reports", "spending")}
          className="btn-ghost text-sm"
        >
          See all
          <ArrowRight className="w-4 h-4 ml-1" />
        </button>
      </div>
      <div className="card-body space-y-3">
        {topCategories.map((category) => {
          const color = TAG_COLORS[category.tag] || TAG_COLORS.Other;
          const barWidth = (category.amount / maxAmount) * 100;

          return (
            <div
              key={category.tag_id}
              className="group cursor-pointer"
              onClick={() =>
                onNavigate("transactions", null, {
                  tag_ids: category.tag_id.toString(),
                  period: "this-month",
                })
              }
            >
              <div className="flex items-center justify-between mb-1">
                <span className="text-sm font-medium text-hone-900 dark:text-hone-100 group-hover:text-hone-600 dark:group-hover:text-hone-300 transition-colors">
                  {category.tag}
                </span>
                <span className="text-sm font-mono text-hone-600 dark:text-hone-400">
                  ${category.amount.toLocaleString(undefined, { minimumFractionDigits: 0, maximumFractionDigits: 0 })}
                </span>
              </div>
              <div className="h-2 bg-hone-100 dark:bg-hone-700 rounded-full overflow-hidden">
                <div
                  className="h-full rounded-full transition-all duration-300 group-hover:opacity-80"
                  style={{
                    width: `${barWidth}%`,
                    backgroundColor: color,
                  }}
                />
              </div>
            </div>
          );
        })}

        {/* Total */}
        <div className="pt-3 mt-3 border-t border-hone-100 dark:border-hone-700 flex justify-between">
          <span className="text-sm font-medium text-hone-500">Total spent</span>
          <span className="text-sm font-mono font-medium text-hone-900 dark:text-hone-100">
            ${total.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
          </span>
        </div>
      </div>
    </div>
  );
}
