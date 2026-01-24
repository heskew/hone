import { ThumbsUp, ThumbsDown, Edit, RotateCcw, RefreshCw, Undo2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api";
import type { FeedbackStats, UserFeedback, FeedbackType, FeedbackTargetType } from "../../types";

export function FeedbackPage() {
  const [feedback, setFeedback] = useState<UserFeedback[]>([]);
  const [stats, setStats] = useState<FeedbackStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [includeReverted, setIncludeReverted] = useState(false);
  const [filterType, setFilterType] = useState<FeedbackType | "">("");
  const [filterTarget, setFilterTarget] = useState<FeedbackTargetType | "">("");

  useEffect(() => {
    loadData();
  }, [includeReverted, filterType, filterTarget]);

  const loadData = async () => {
    try {
      setLoading(true);
      const [feedbackData, statsData] = await Promise.all([
        api.getFeedback({
          include_reverted: includeReverted,
          feedback_type: filterType || undefined,
          target_type: filterTarget || undefined,
          limit: 100,
        }),
        api.getFeedbackStats(),
      ]);
      setFeedback(feedbackData);
      setStats(statsData);
    } catch (err) {
      console.error("Failed to load feedback:", err);
    } finally {
      setLoading(false);
    }
  };

  const handleRevert = async (id: number) => {
    try {
      await api.revertFeedback(id);
      loadData();
    } catch (err) {
      console.error("Failed to revert feedback:", err);
    }
  };

  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  };

  const getFeedbackIcon = (type: FeedbackType) => {
    switch (type) {
      case "helpful":
        return <ThumbsUp className="w-4 h-4 text-savings-dark" />;
      case "not_helpful":
        return <ThumbsDown className="w-4 h-4 text-waste" />;
      case "correction":
        return <Edit className="w-4 h-4 text-attention" />;
      case "dismissal":
        return <RotateCcw className="w-4 h-4 text-hone-500" />;
    }
  };

  const getFeedbackLabel = (type: FeedbackType) => {
    switch (type) {
      case "helpful":
        return "Helpful";
      case "not_helpful":
        return "Not Helpful";
      case "correction":
        return "Correction";
      case "dismissal":
        return "Dismissed";
    }
  };

  const getTargetLabel = (type: FeedbackTargetType) => {
    switch (type) {
      case "alert":
        return "Alert";
      case "insight":
        return "Insight";
      case "classification":
        return "Classification";
      case "explanation":
        return "Explanation";
      case "receipt_match":
        return "Receipt Match";
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-hone-900 dark:text-hone-50">
          Feedback History
        </h1>
        <button onClick={loadData} className="btn-ghost" title="Refresh">
          <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
        </button>
      </div>

      {/* Stats Summary */}
      {stats && (
        <div className="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-6 gap-4">
          <div className="card p-4">
            <p className="text-xs text-hone-500 uppercase tracking-wide">Total</p>
            <p className="text-2xl font-bold text-hone-900 dark:text-hone-50">
              {stats.total_feedback}
            </p>
          </div>
          <div className="card p-4">
            <p className="text-xs text-hone-500 uppercase tracking-wide">Helpful</p>
            <p className="text-2xl font-bold text-savings-dark">
              {stats.helpful_count}
            </p>
          </div>
          <div className="card p-4">
            <p className="text-xs text-hone-500 uppercase tracking-wide">Not Helpful</p>
            <p className="text-2xl font-bold text-waste">
              {stats.not_helpful_count}
            </p>
          </div>
          <div className="card p-4">
            <p className="text-xs text-hone-500 uppercase tracking-wide">Corrections</p>
            <p className="text-2xl font-bold text-attention">
              {stats.correction_count}
            </p>
          </div>
          <div className="card p-4">
            <p className="text-xs text-hone-500 uppercase tracking-wide">Dismissals</p>
            <p className="text-2xl font-bold text-hone-600 dark:text-hone-300">
              {stats.dismissal_count}
            </p>
          </div>
          <div className="card p-4">
            <p className="text-xs text-hone-500 uppercase tracking-wide">Reverted</p>
            <p className="text-2xl font-bold text-hone-400">
              {stats.reverted_count}
            </p>
          </div>
        </div>
      )}

      {/* Helpfulness by Target Type */}
      {stats && stats.by_target_type.length > 0 && (
        <div className="card">
          <div className="card-header">
            <h2 className="text-lg font-semibold text-hone-900 dark:text-hone-50">
              Helpfulness by Content Type
            </h2>
          </div>
          <div className="card-body">
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
              {stats.by_target_type.map((targetStats) => (
                <div
                  key={targetStats.target_type}
                  className="p-3 bg-hone-50 dark:bg-hone-800/50 rounded-lg"
                >
                  <p className="font-medium text-hone-900 dark:text-hone-100 mb-2">
                    {getTargetLabel(targetStats.target_type)}
                  </p>
                  <div className="flex items-center gap-4 text-sm">
                    <span className="text-savings-dark">
                      {targetStats.helpful} helpful
                    </span>
                    <span className="text-waste">
                      {targetStats.not_helpful} not helpful
                    </span>
                  </div>
                  <div className="mt-2 h-2 bg-hone-200 dark:bg-hone-700 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-savings transition-all"
                      style={{ width: `${targetStats.helpfulness_ratio * 100}%` }}
                    />
                  </div>
                  <p className="text-xs text-hone-500 mt-1">
                    {(targetStats.helpfulness_ratio * 100).toFixed(0)}% helpful
                  </p>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Filters */}
      <div className="flex flex-wrap items-center gap-4">
        <select
          value={filterType}
          onChange={(e) => setFilterType(e.target.value as FeedbackType | "")}
          className="input-field w-auto"
        >
          <option value="">All Types</option>
          <option value="helpful">Helpful</option>
          <option value="not_helpful">Not Helpful</option>
          <option value="correction">Correction</option>
          <option value="dismissal">Dismissal</option>
        </select>

        <select
          value={filterTarget}
          onChange={(e) => setFilterTarget(e.target.value as FeedbackTargetType | "")}
          className="input-field w-auto"
        >
          <option value="">All Targets</option>
          <option value="alert">Alert</option>
          <option value="insight">Insight</option>
          <option value="classification">Classification</option>
          <option value="explanation">Explanation</option>
          <option value="receipt_match">Receipt Match</option>
        </select>

        <label className="flex items-center gap-2 text-sm text-hone-600 dark:text-hone-300">
          <input
            type="checkbox"
            checked={includeReverted}
            onChange={(e) => setIncludeReverted(e.target.checked)}
            className="rounded border-hone-300 dark:border-hone-600"
          />
          Include reverted
        </label>
      </div>

      {/* Feedback List */}
      <div className="card">
        <div className="card-header">
          <h2 className="text-lg font-semibold text-hone-900 dark:text-hone-50">
            Recent Feedback
          </h2>
        </div>
        <div className="divide-y divide-hone-200 dark:divide-hone-700">
          {loading ? (
            <div className="flex items-center justify-center py-8">
              <RefreshCw className="w-6 h-6 text-hone-400 animate-spin" />
            </div>
          ) : feedback.length === 0 ? (
            <p className="text-center py-8 text-hone-500">
              No feedback recorded yet. Rate AI-generated explanations in alerts to start collecting feedback.
            </p>
          ) : (
            feedback.map((item) => (
              <div
                key={item.id}
                className={`px-4 py-3 hover:bg-hone-50 dark:hover:bg-hone-800/50 ${
                  item.reverted_at ? "opacity-50" : ""
                }`}
              >
                <div className="flex items-start justify-between gap-4">
                  <div className="flex items-start gap-3">
                    <div className="mt-1">{getFeedbackIcon(item.feedback_type)}</div>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-hone-900 dark:text-hone-100">
                          {getFeedbackLabel(item.feedback_type)}
                        </span>
                        <span className="text-xs px-2 py-0.5 bg-hone-100 dark:bg-hone-700 text-hone-600 dark:text-hone-300 rounded">
                          {getTargetLabel(item.target_type)}
                        </span>
                        {item.reverted_at && (
                          <span className="text-xs px-2 py-0.5 bg-hone-200 dark:bg-hone-600 text-hone-500 dark:text-hone-400 rounded">
                            Reverted
                          </span>
                        )}
                      </div>
                      {item.reason && (
                        <p className="text-sm text-hone-600 dark:text-hone-300 mt-1">
                          "{item.reason}"
                        </p>
                      )}
                      {item.context?.model && (
                        <p className="text-xs text-hone-500 mt-1">
                          Model: {item.context.model}
                        </p>
                      )}
                      <p className="text-xs text-hone-400 mt-1">
                        {formatDate(item.created_at)}
                      </p>
                    </div>
                  </div>
                  {!item.reverted_at && (
                    <button
                      onClick={() => handleRevert(item.id)}
                      className="btn-ghost text-xs"
                      title="Undo this feedback"
                    >
                      <Undo2 className="w-4 h-4" />
                    </button>
                  )}
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
