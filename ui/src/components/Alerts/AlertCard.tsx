import { Ghost, TrendingUp, Users, RotateCcw, ChevronRight, BarChart3 } from "lucide-react";
import { useState } from "react";
import type { Alert } from "../../types";
import { AlertDetailModal } from "./AlertDetailModal";

interface AlertCardProps {
  alert: Alert;
  onDismiss: (id: number) => void;
  onAcknowledge: (id: number) => void;
  onRestore?: (id: number) => void;
  onExclude?: (id: number) => void;
  onCancel?: (id: number) => void;
}

export function AlertCard({ alert, onDismiss, onAcknowledge, onRestore, onExclude, onCancel }: AlertCardProps) {
  const [showDetail, setShowDetail] = useState(false);

  const getCardClass = () => {
    if (alert.dismissed) {
      return "alert-card-dismissed cursor-pointer";
    }
    switch (alert.alert_type) {
      case "zombie":
        return "alert-card-zombie cursor-pointer";
      case "price_increase":
        return "alert-card-increase cursor-pointer";
      case "duplicate":
        return "alert-card-duplicate cursor-pointer";
      case "resume":
        return "alert-card-resume cursor-pointer";
      case "spending_anomaly":
        return "alert-card-zombie cursor-pointer"; // Same style as zombie for now
    }
  };

  const getIcon = () => {
    switch (alert.alert_type) {
      case "zombie":
        return <Ghost className={`w-5 h-5 ${alert.dismissed ? "text-hone-400" : "text-attention"}`} />;
      case "price_increase":
        return <TrendingUp className={`w-5 h-5 ${alert.dismissed ? "text-hone-400" : "text-waste"}`} />;
      case "duplicate":
        return <Users className="w-5 h-5 text-hone-500" />;
      case "resume":
        return <RotateCcw className={`w-5 h-5 ${alert.dismissed ? "text-hone-400" : "text-waste"}`} />;
      case "spending_anomaly":
        return <BarChart3 className={`w-5 h-5 ${alert.dismissed ? "text-hone-400" : "text-attention"}`} />;
    }
  };

  const getLabel = () => {
    switch (alert.alert_type) {
      case "zombie":
        return "Zombie Subscription";
      case "price_increase":
        return "Price Increase";
      case "duplicate":
        return "Duplicate Service";
      case "resume":
        return "Subscription Resumed";
      case "spending_anomaly":
        return "Spending Change";
    }
  };

  const isAcknowledged = alert.dismissed && alert.subscription?.user_acknowledged;

  const handleCardClick = (e: React.MouseEvent) => {
    // Don't open detail if clicking a button
    if ((e.target as HTMLElement).closest("button")) {
      return;
    }
    setShowDetail(true);
  };

  return (
    <>
      <div className={getCardClass()} onClick={handleCardClick}>
        <div className="flex-shrink-0">{getIcon()}</div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <span className={`font-semibold ${alert.dismissed ? "text-hone-500 dark:text-hone-400" : "text-hone-900 dark:text-hone-50"}`}>{getLabel()}</span>
            {alert.subscription && <span className="badge-neutral">{alert.subscription.merchant}</span>}
            {alert.spending_anomaly && <span className="badge-neutral">{alert.spending_anomaly.tag_name}</span>}
            {isAcknowledged && <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-savings/20 text-savings dark:text-savings-light">Acknowledged</span>}
            {alert.dismissed && !isAcknowledged && <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-hone-200 dark:bg-hone-600 text-hone-600 dark:text-hone-200">Dismissed</span>}
          </div>
          {alert.message && <p className={`text-sm ${alert.dismissed ? "text-hone-500 dark:text-hone-400" : "text-hone-600 dark:text-hone-300"}`}>{alert.message}</p>}
        </div>

        {!alert.dismissed ? (
          <div className="flex items-center gap-2 flex-shrink-0">
            {alert.subscription_id && alert.alert_type === "zombie" && (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onAcknowledge(alert.subscription_id!);
                }}
                className="btn-success text-sm py-1.5 px-3"
              >
                I still use this
              </button>
            )}
            <button
              onClick={(e) => {
                e.stopPropagation();
                onDismiss(alert.id);
              }}
              className="btn-secondary text-sm py-1.5 px-3"
            >
              Dismiss
            </button>
            <ChevronRight className="w-5 h-5 text-hone-400" />
          </div>
        ) : onRestore ? (
          <div className="flex items-center gap-2 flex-shrink-0">
            <button
              onClick={(e) => {
                e.stopPropagation();
                onRestore(alert.id);
              }}
              className="btn-secondary text-sm py-1.5 px-3"
            >
              Restore
            </button>
            <ChevronRight className="w-5 h-5 text-hone-400" />
          </div>
        ) : (
          <ChevronRight className="w-5 h-5 text-hone-400 flex-shrink-0" />
        )}
      </div>

      {showDetail && (
        <AlertDetailModal
          alert={alert}
          onClose={() => setShowDetail(false)}
          onDismiss={onDismiss}
          onAcknowledge={onAcknowledge}
          onExclude={onExclude}
          onCancel={onCancel}
        />
      )}
    </>
  );
}
