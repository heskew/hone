import { Ghost, TrendingUp, Users, X } from "lucide-react";
import type { Alert } from "../../types";

interface AlertItemProps {
  alert: Alert;
  onDismiss: (id: number) => void;
}

export function AlertItem({ alert, onDismiss }: AlertItemProps) {
  const getAlertIcon = () => {
    switch (alert.alert_type) {
      case "zombie":
        return <Ghost className="w-5 h-5 text-attention" />;
      case "price_increase":
        return <TrendingUp className="w-5 h-5 text-waste" />;
      case "duplicate":
        return <Users className="w-5 h-5 text-hone-500" />;
    }
  };

  const getAlertLabel = () => {
    switch (alert.alert_type) {
      case "zombie":
        return "Zombie Subscription";
      case "price_increase":
        return "Price Increase";
      case "duplicate":
        return "Duplicate Service";
    }
  };

  return (
    <div className="p-4 flex items-start gap-4 hover:bg-hone-50 dark:hover:bg-hone-800 transition-colors">
      <div className="flex-shrink-0 mt-0.5">{getAlertIcon()}</div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-hone-700 dark:text-hone-300">{getAlertLabel()}</span>
          {alert.subscription && <span className="badge-neutral">{alert.subscription.merchant}</span>}
        </div>
        {alert.message && <p className="text-sm text-hone-500 dark:text-hone-400 mt-1">{alert.message}</p>}
      </div>
      <button
        onClick={() => onDismiss(alert.id)}
        className="flex-shrink-0 p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-300 hover:bg-hone-100 dark:hover:bg-hone-700 rounded transition-colors"
        title="Dismiss"
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  );
}
