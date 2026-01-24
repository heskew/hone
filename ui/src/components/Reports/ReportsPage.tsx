import { useState, useEffect } from "react";
import { BarChart3, X } from "lucide-react";
import type { Entity, ReportTab, Period, DateRange } from "../../types";
import { api } from "../../api";
import { SpendingTab } from "./SpendingTab";
import { TrendsTab } from "./TrendsTab";
import { MerchantsTab } from "./MerchantsTab";
import { SubscriptionsTab } from "./SubscriptionsTab";
import { useHashRouter } from "../../hooks";

const VALID_TABS: ReportTab[] = ["spending", "trends", "merchants", "subscriptions"];
const VALID_PERIODS: Period[] = ["this-month", "last-month", "last-30-days", "last-90-days", "this-year", "last-year", "all", "custom"];

export function ReportsPage() {
  const { state, setSubview, updateParams } = useHashRouter();

  // Read tab from URL subview, default to spending
  const activeTab: ReportTab = VALID_TABS.includes(state.subview as ReportTab)
    ? (state.subview as ReportTab)
    : "spending";

  // Read period from URL params, default to this-month
  const period: Period = VALID_PERIODS.includes(state.params.period as Period)
    ? (state.params.period as Period)
    : "this-month";

  // Read custom range from URL params
  const appliedCustomRange: DateRange = {
    from: state.params.from,
    to: state.params.to,
  };

  // Local state for custom range input (before applying)
  const [customRange, setCustomRange] = useState<DateRange>(appliedCustomRange);

  // Entity and card member filters
  const [entities, setEntities] = useState<Entity[]>([]);
  const [cardMembers, setCardMembers] = useState<string[]>([]);
  const [selectedEntityId, setSelectedEntityId] = useState<number | null>(null);
  const [selectedCardMember, setSelectedCardMember] = useState<string | null>(null);

  // Load entities and card members on mount
  useEffect(() => {
    api.getEntities({ entity_type: "person" }).then(setEntities).catch(console.error);
    // Fetch transactions to get unique card members
    api.getTransactions({ limit: 100 }).then((result) => {
      const members = new Set<string>();
      result.transactions.forEach((t) => {
        if (t.card_member) members.add(t.card_member);
      });
      if (members.size > 0) {
        setCardMembers(Array.from(members).sort());
      }
    }).catch(console.error);
  }, []);

  // Sync custom range input when URL changes
  useEffect(() => {
    if (state.params.from && state.params.to) {
      setCustomRange({ from: state.params.from, to: state.params.to });
    }
  }, [state.params.from, state.params.to]);

  const handleTabChange = (tab: ReportTab) => {
    setSubview(tab);
  };

  const handlePeriodChange = (newPeriod: Period) => {
    if (newPeriod === "custom") {
      updateParams({ period: newPeriod });
    } else {
      // Clear from/to when switching away from custom
      updateParams({ period: newPeriod, from: "", to: "" });
    }
  };

  const handleApplyCustomRange = () => {
    if (customRange.from && customRange.to) {
      updateParams({ period: "custom", from: customRange.from, to: customRange.to });
    }
  };

  // Build the period params to pass to tabs
  const periodParams = period === "custom" && appliedCustomRange.from && appliedCustomRange.to
    ? { from: appliedCustomRange.from, to: appliedCustomRange.to }
    : { period };

  // Build filter params to pass to tabs
  const filterParams = {
    entity_id: selectedEntityId || undefined,
    card_member: selectedCardMember || undefined,
  };

  // Show entity/card_member filters for non-subscription tabs
  const showPersonFilters = activeTab !== "subscriptions" && (entities.length > 0 || cardMembers.length > 0);

  return (
    <div className="space-y-6">
      {/* Header with period selector */}
      <div className="flex items-center justify-between flex-wrap gap-4">
        <div className="flex items-center gap-3">
          <BarChart3 className="w-6 h-6 text-hone-600" />
          <h1 className="text-2xl font-bold text-hone-900 dark:text-hone-50">Reports</h1>
        </div>
        <div className="flex items-center gap-2 flex-wrap">
          <select
            value={period}
            onChange={(e) => handlePeriodChange(e.target.value as Period)}
            className="input py-2"
          >
            <option value="this-month">This Month</option>
            <option value="last-month">Last Month</option>
            <option value="last-30-days">Last 30 Days</option>
            <option value="last-90-days">Last 90 Days</option>
            <option value="this-year">This Year</option>
            <option value="last-year">Last Year</option>
            <option value="all">All Time</option>
            <option value="custom">Custom Range</option>
          </select>
          {period === "custom" && (
            <div className="flex items-center gap-2">
              <input
                type="date"
                value={customRange.from || ""}
                onChange={(e) => setCustomRange({ ...customRange, from: e.target.value })}
                className="input py-2"
              />
              <span className="text-hone-400">to</span>
              <input
                type="date"
                value={customRange.to || ""}
                onChange={(e) => setCustomRange({ ...customRange, to: e.target.value })}
                className="input py-2"
              />
              <button
                onClick={handleApplyCustomRange}
                disabled={!customRange.from || !customRange.to}
                className="btn-primary py-2 px-4 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Apply
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Entity and card member filters */}
      {showPersonFilters && (
        <div className="flex items-center gap-4 flex-wrap">
          {/* Entity (person) filter */}
          {entities.length > 0 && (
            <div className="flex items-center gap-2">
              <select
                value={selectedEntityId ?? ""}
                onChange={(e) => setSelectedEntityId(e.target.value ? Number(e.target.value) : null)}
                className={`pl-3 pr-8 py-2 border rounded-lg transition-colors ${
                  selectedEntityId
                    ? "border-hone-500 bg-hone-100 dark:bg-hone-700 text-hone-900 dark:text-hone-50"
                    : "border-hone-200 dark:border-hone-600 bg-white dark:bg-hone-800 text-hone-600 dark:text-hone-300"
                }`}
              >
                <option value="">All people</option>
                {entities.map((entity) => (
                  <option key={entity.id} value={entity.id}>
                    {entity.name}
                  </option>
                ))}
              </select>
              {selectedEntityId && (
                <button
                  onClick={() => setSelectedEntityId(null)}
                  className="p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200"
                  title="Clear person filter"
                >
                  <X className="w-4 h-4" />
                </button>
              )}
            </div>
          )}

          {/* Card member filter */}
          {cardMembers.length > 0 && (
            <div className="flex items-center gap-2">
              <select
                value={selectedCardMember ?? ""}
                onChange={(e) => setSelectedCardMember(e.target.value || null)}
                className={`pl-3 pr-8 py-2 border rounded-lg transition-colors ${
                  selectedCardMember
                    ? "border-hone-500 bg-hone-100 dark:bg-hone-700 text-hone-900 dark:text-hone-50"
                    : "border-hone-200 dark:border-hone-600 bg-white dark:bg-hone-800 text-hone-600 dark:text-hone-300"
                }`}
              >
                <option value="">All cardholders</option>
                {cardMembers.map((member) => (
                  <option key={member} value={member}>
                    {member}
                  </option>
                ))}
              </select>
              {selectedCardMember && (
                <button
                  onClick={() => setSelectedCardMember(null)}
                  className="p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200"
                  title="Clear cardholder filter"
                >
                  <X className="w-4 h-4" />
                </button>
              )}
            </div>
          )}
        </div>
      )}

      {/* Tab navigation */}
      <div className="flex gap-1 border-b border-hone-200 dark:border-hone-700">
        {(["spending", "trends", "merchants", "subscriptions"] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => handleTabChange(tab)}
            className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
              activeTab === tab
                ? "border-hone-600 text-hone-900 dark:text-hone-50"
                : "border-transparent text-hone-500 hover:text-hone-700 dark:text-hone-400 dark:hover:text-hone-200"
            }`}
          >
            {tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </div>

      {/* Tab content */}
      {activeTab === "spending" && <SpendingTab periodParams={periodParams} filterParams={filterParams} />}
      {activeTab === "trends" && <TrendsTab periodParams={periodParams} filterParams={filterParams} />}
      {activeTab === "merchants" && <MerchantsTab periodParams={periodParams} filterParams={filterParams} />}
      {activeTab === "subscriptions" && <SubscriptionsTab />}
    </div>
  );
}
