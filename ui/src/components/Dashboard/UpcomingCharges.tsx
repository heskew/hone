import { ArrowRight, Calendar, RefreshCw } from "lucide-react";
import type { Subscription } from "../../types";
import type { View } from "../../hooks";

interface UpcomingCharge {
  subscription: Subscription;
  expectedDate: Date;
  daysUntil: number;
}

interface UpcomingChargesProps {
  subscriptions: Subscription[];
  onNavigate: (view: View, subview?: string | null, params?: Record<string, string>) => void;
}

function calculateNextChargeDate(subscription: Subscription): Date | null {
  if (!subscription.last_seen || !subscription.frequency) return null;

  const lastSeen = new Date(subscription.last_seen);
  const today = new Date();
  today.setHours(0, 0, 0, 0);

  // Calculate the next expected charge based on frequency
  const nextCharge = new Date(lastSeen);

  while (nextCharge <= today) {
    switch (subscription.frequency) {
      case "weekly":
        nextCharge.setDate(nextCharge.getDate() + 7);
        break;
      case "monthly":
        nextCharge.setMonth(nextCharge.getMonth() + 1);
        break;
      case "yearly":
        nextCharge.setFullYear(nextCharge.getFullYear() + 1);
        break;
    }
  }

  return nextCharge;
}

export function UpcomingCharges({ subscriptions, onNavigate }: UpcomingChargesProps) {
  // Filter to active subscriptions and calculate upcoming charges
  const today = new Date();
  today.setHours(0, 0, 0, 0);

  const upcomingCharges: UpcomingCharge[] = subscriptions
    .filter((s) => s.status === "active" && s.amount && s.frequency)
    .map((subscription) => {
      const expectedDate = calculateNextChargeDate(subscription);
      if (!expectedDate) return null;

      const daysUntil = Math.ceil((expectedDate.getTime() - today.getTime()) / (1000 * 60 * 60 * 24));
      return { subscription, expectedDate, daysUntil };
    })
    .filter((c): c is UpcomingCharge => c !== null && c.daysUntil >= 0 && c.daysUntil <= 14)
    .sort((a, b) => a.daysUntil - b.daysUntil)
    .slice(0, 5);

  if (upcomingCharges.length === 0) {
    return (
      <div className="card">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold flex items-center gap-2">
            <Calendar className="w-5 h-5 text-hone-500" />
            Upcoming Charges
          </h2>
          <button
            onClick={() => onNavigate("subscriptions")}
            className="btn-ghost text-sm"
          >
            View all
            <ArrowRight className="w-4 h-4 ml-1" />
          </button>
        </div>
        <div className="card-body text-center py-8">
          <p className="text-hone-500">No upcoming charges in the next 2 weeks</p>
        </div>
      </div>
    );
  }

  const formatDaysUntil = (days: number) => {
    if (days === 0) return "Today";
    if (days === 1) return "Tomorrow";
    return `In ${days} days`;
  };

  const totalUpcoming = upcomingCharges.reduce(
    (sum, c) => sum + (c.subscription.amount || 0),
    0
  );

  return (
    <div className="card">
      <div className="card-header flex items-center justify-between">
        <h2 className="text-lg font-semibold flex items-center gap-2">
          <Calendar className="w-5 h-5 text-hone-500" />
          Upcoming Charges
        </h2>
        <button
          onClick={() => onNavigate("subscriptions")}
          className="btn-ghost text-sm"
        >
          View all
          <ArrowRight className="w-4 h-4 ml-1" />
        </button>
      </div>
      <div className="divide-y divide-hone-100 dark:divide-hone-700">
        {upcomingCharges.map(({ subscription, expectedDate, daysUntil }) => (
          <div
            key={subscription.id}
            className="px-4 py-3 flex items-center justify-between hover:bg-hone-50 dark:hover:bg-hone-800 cursor-pointer transition-colors"
            onClick={() => onNavigate("subscriptions", subscription.id.toString())}
          >
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-full bg-hone-100 dark:bg-hone-700 flex items-center justify-center">
                <RefreshCw className="w-4 h-4 text-hone-500" />
              </div>
              <div>
                <div className="font-medium text-hone-900 dark:text-hone-100">
                  {subscription.merchant}
                </div>
                <div className={`text-sm ${daysUntil <= 2 ? "text-attention font-medium" : "text-hone-500"}`}>
                  {formatDaysUntil(daysUntil)}
                  <span className="text-hone-400 ml-1">
                    · {expectedDate.toLocaleDateString("en-US", { month: "short", day: "numeric" })}
                  </span>
                </div>
              </div>
            </div>
            <div className="font-mono font-medium text-hone-900 dark:text-hone-100">
              ${subscription.amount?.toFixed(2)}
            </div>
          </div>
        ))}
      </div>
      {/* Total upcoming */}
      <div className="px-4 py-3 bg-hone-50 dark:bg-hone-800/50 flex justify-between text-sm">
        <span className="text-hone-600 dark:text-hone-400">
          Next 2 weeks
        </span>
        <span className="font-mono font-medium text-hone-900 dark:text-hone-100">
          ${totalUpcoming.toFixed(2)}
        </span>
      </div>
    </div>
  );
}
