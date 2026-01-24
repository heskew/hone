import { BadgeDollarSign, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api";
import type { Alert } from "../../types";
import { AlertCard } from "./AlertCard";

interface AlertsListProps {
  alerts: Alert[];
  onDismiss: (id: number) => void;
  onAcknowledge: (id: number) => void;
  onRestore?: (id: number) => void;
  onExclude?: (id: number) => void;
  onCancel?: (id: number) => void;
}

export function AlertsList({
  alerts: activeAlerts,
  onDismiss,
  onAcknowledge,
  onRestore,
  onExclude,
  onCancel,
}: AlertsListProps) {
  const [showDismissed, setShowDismissed] = useState(false);
  const [allAlerts, setAllAlerts] = useState<Alert[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (showDismissed) {
      loadAllAlerts();
    }
  }, [showDismissed]);

  const loadAllAlerts = async () => {
    try {
      setLoading(true);
      const alerts = await api.getAlerts(true);
      setAllAlerts(alerts);
    } catch (err) {
      console.error("Failed to load alerts:", err);
    } finally {
      setLoading(false);
    }
  };

  const handleRestore = async (id: number) => {
    try {
      await api.restoreAlert(id);
      // Remove from dismissed list
      setAllAlerts(allAlerts.filter(a => a.id !== id));
      // Notify parent to refresh
      onRestore?.(id);
    } catch (err) {
      console.error("Failed to restore alert:", err);
    }
  };

  const dismissedAlerts = showDismissed ? allAlerts.filter(a => a.dismissed) : [];
  const dismissedCount = showDismissed ? dismissedAlerts.length : 0;

  return (
    <div className="space-y-6 animate-fade-in">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-hone-900 dark:text-hone-50">Alerts</h1>
        <label className="flex items-center gap-2 text-sm text-hone-600 dark:text-hone-400 cursor-pointer">
          <input
            type="checkbox"
            checked={showDismissed}
            onChange={(e) => setShowDismissed(e.target.checked)}
            className="rounded border-hone-300 dark:border-hone-600 text-hone-600 focus:ring-hone-500 dark:bg-hone-700"
          />
          Show dismissed
          {showDismissed && dismissedCount > 0 && (
            <span className="text-hone-400">({dismissedCount})</span>
          )}
        </label>
      </div>

      {loading ? (
        <div className="card p-8 text-center">
          <RefreshCw className="w-8 h-8 text-hone-300 mx-auto mb-4 animate-spin" />
          <p className="text-hone-500 dark:text-hone-400">Loading alerts...</p>
        </div>
      ) : activeAlerts.length === 0 && !showDismissed ? (
        <div className="card p-8 text-center">
          <div className="w-16 h-16 bg-savings-light dark:bg-savings-dark/20 rounded-full flex items-center justify-center mx-auto mb-4">
            <BadgeDollarSign className="w-8 h-8 text-savings" />
          </div>
          <p className="text-xl font-semibold mb-2 text-hone-900 dark:text-hone-50">All Clear!</p>
          <p className="text-hone-500 dark:text-hone-400">No alerts to review. Your spending looks healthy.</p>
        </div>
      ) : (
        <>
          {/* Active alerts */}
          {activeAlerts.length > 0 && (
            <div className="space-y-4">
              {activeAlerts.map((alert) => (
                <AlertCard
                  key={alert.id}
                  alert={alert}
                  onDismiss={onDismiss}
                  onAcknowledge={onAcknowledge}
                  onExclude={onExclude}
                  onCancel={onCancel}
                />
              ))}
            </div>
          )}

          {/* Empty state for active alerts when showing dismissed */}
          {activeAlerts.length === 0 && showDismissed && (
            <div className="card p-6 text-center">
              <p className="text-hone-500 dark:text-hone-400">No active alerts</p>
            </div>
          )}

          {/* Dismissed alerts section */}
          {showDismissed && dismissedAlerts.length > 0 && (
            <div className="space-y-4 pt-4">
              <h2 className="text-lg font-semibold text-hone-500 dark:text-hone-400 border-t border-hone-200 dark:border-hone-700 pt-6">
                Dismissed ({dismissedCount})
              </h2>
              {dismissedAlerts.map((alert) => (
                <AlertCard
                  key={alert.id}
                  alert={alert}
                  onDismiss={onDismiss}
                  onAcknowledge={onAcknowledge}
                  onRestore={handleRestore}
                />
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
