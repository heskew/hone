import { Lightbulb, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api";
import type { InsightFinding } from "../../types";
import { InsightCard } from "./InsightCard";

interface InsightsWidgetProps {
  limit?: number;
  onRefresh?: () => void;
}

export function InsightsWidget({ limit = 5, onRefresh }: InsightsWidgetProps) {
  const [insights, setInsights] = useState<InsightFinding[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchInsights = async () => {
    try {
      const data = await api.getTopInsights(limit);
      setInsights(data);
      setError(null);
    } catch (err) {
      console.error("Failed to fetch insights:", err);
      setError("Failed to load insights");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchInsights();
  }, [limit]);

  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      await api.refreshInsights();
      await fetchInsights();
      onRefresh?.();
    } catch (err) {
      console.error("Failed to refresh insights:", err);
    } finally {
      setRefreshing(false);
    }
  };

  const handleInsightChange = () => {
    fetchInsights();
  };

  if (loading) {
    return (
      <div className="card">
        <div className="card-header">
          <h2 className="text-lg font-semibold flex items-center gap-2">
            <Lightbulb className="w-5 h-5 text-amber-500" />
            What's Going On
          </h2>
        </div>
        <div className="card-body">
          <div className="text-center py-8 text-hone-500">Loading insights...</div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="card">
        <div className="card-header">
          <h2 className="text-lg font-semibold flex items-center gap-2">
            <Lightbulb className="w-5 h-5 text-amber-500" />
            What's Going On
          </h2>
        </div>
        <div className="card-body">
          <div className="text-center py-8 text-waste">{error}</div>
        </div>
      </div>
    );
  }

  return (
    <div className="card">
      <div className="card-header flex items-center justify-between">
        <h2 className="text-lg font-semibold flex items-center gap-2">
          <Lightbulb className="w-5 h-5 text-amber-500" />
          What's Going On
        </h2>
        <button
          onClick={handleRefresh}
          disabled={refreshing}
          className="p-1.5 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200 rounded disabled:opacity-50"
          title="Refresh insights"
        >
          <RefreshCw className={`w-4 h-4 ${refreshing ? "animate-spin" : ""}`} />
        </button>
      </div>

      <div className="card-body">
        {insights.length === 0 ? (
          <div className="text-center py-8 text-hone-500">
            <Lightbulb className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p>No insights right now</p>
            <p className="text-sm mt-1">Everything looks good!</p>
          </div>
        ) : (
          <div className="space-y-3">
            {insights.map((insight) => (
              <InsightCard
                key={insight.id}
                insight={insight}
                onDismiss={handleInsightChange}
                onSnooze={handleInsightChange}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
