import { useState } from "react";
import type { Alert, Subscription } from "../../types";
import { SubscriptionDetailModal } from "./SubscriptionDetailModal";

interface SubscriptionItemProps {
  subscription: Subscription;
  accountName: string | null;
  alerts: Alert[];
  onAcknowledge: (id: number) => void;
  onCancel: (id: number) => void;
  onExclude: (id: number) => void;
  onUnexclude: (id: number) => void;
}

export function SubscriptionItem({
  subscription,
  accountName,
  alerts,
  onAcknowledge,
  onCancel,
  onExclude,
  onUnexclude,
}: SubscriptionItemProps) {
  const [showModal, setShowModal] = useState(false);

  const formatFrequency = (freq: string | null) => {
    switch (freq) {
      case "weekly":
        return "/week";
      case "monthly":
        return "/mo";
      case "yearly":
        return "/year";
      default:
        return "";
    }
  };

  const handleClick = (e: React.MouseEvent) => {
    // Don't open modal if clicking on a button
    if ((e.target as HTMLElement).closest("button")) {
      return;
    }
    setShowModal(true);
  };

  return (
    <>
      <div
        className="p-4 flex items-center justify-between hover:bg-hone-100 dark:hover:bg-hone-700 transition-colors cursor-pointer"
        onClick={handleClick}
      >
        <div className="flex items-center gap-4">
          <div>
            <div className="font-medium">{subscription.merchant}</div>
            <div className="flex items-center gap-2 text-sm text-hone-400">
              {subscription.first_seen && (
                <span>
                  Since{" "}
                  {(() => {
                    const [year, month, day] = subscription.first_seen.split("-").map(Number);
                    return new Date(year, month - 1, day, 12, 0, 0).toLocaleDateString("en-US", {
                      month: "short",
                      year: "numeric",
                    });
                  })()}
                </span>
              )}
              {accountName && (
                <>
                  {subscription.first_seen && <span>·</span>}
                  <span>{accountName}</span>
                </>
              )}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-4">
          {subscription.amount && (
            <div className="text-right">
              <div className="amount-negative font-semibold">
                ${subscription.amount.toFixed(2)}
                <span className="text-hone-400 text-sm font-normal">
                  {formatFrequency(subscription.frequency)}
                </span>
              </div>
            </div>
          )}

          {subscription.status === "zombie" && !subscription.user_acknowledged && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onAcknowledge(subscription.id);
              }}
              className="btn-secondary text-sm"
            >
              I use this
            </button>
          )}

          {subscription.status === "excluded" && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onUnexclude(subscription.id);
              }}
              className="btn-secondary text-sm"
            >
              Include
            </button>
          )}

          {subscription.user_acknowledged && <span className="badge-success">Acknowledged</span>}
          {subscription.status === "excluded" && <span className="badge-neutral">Excluded</span>}
        </div>
      </div>

      {showModal && (
        <SubscriptionDetailModal
          subscription={subscription}
          accountName={accountName}
          alerts={alerts}
          onClose={() => setShowModal(false)}
          onAcknowledge={onAcknowledge}
          onCancel={onCancel}
          onExclude={onExclude}
          onUnexclude={onUnexclude}
        />
      )}
    </>
  );
}
