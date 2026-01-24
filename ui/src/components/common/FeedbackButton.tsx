import { ThumbsUp, ThumbsDown } from "lucide-react";
import { useState } from "react";
import { api } from "../../api";
import type { FeedbackTargetType, UserFeedback } from "../../types";

interface FeedbackButtonProps {
  targetType: FeedbackTargetType;
  targetId: number;
  /** Optional: existing feedback to show current state */
  existingFeedback?: UserFeedback[];
  /** Callback when feedback is submitted */
  onFeedback?: (feedback: UserFeedback) => void;
  /** Size variant */
  size?: "sm" | "md";
}

export function FeedbackButton({
  targetType,
  targetId,
  existingFeedback,
  onFeedback,
  size = "sm",
}: FeedbackButtonProps) {
  const [submitting, setSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState<"helpful" | "not_helpful" | null>(null);

  // Check if user already submitted feedback for this target
  const activeFeedback = existingFeedback?.find(
    (f) => f.target_id === targetId && !f.reverted_at
  );

  const currentState = submitted || (activeFeedback?.feedback_type as "helpful" | "not_helpful" | null);

  const handleFeedback = async (helpful: boolean) => {
    if (submitting) return;

    try {
      setSubmitting(true);

      // Use the convenience endpoint for alerts
      if (targetType === "alert" || targetType === "explanation") {
        const response = await api.rateAlert(targetId, helpful);
        setSubmitted(helpful ? "helpful" : "not_helpful");
        if (onFeedback) {
          onFeedback(response.feedback);
        }
      } else {
        // Use the generic feedback endpoint
        const response = await api.createFeedback({
          feedback_type: helpful ? "helpful" : "not_helpful",
          target_type: targetType,
          target_id: targetId,
        });
        setSubmitted(helpful ? "helpful" : "not_helpful");
        if (onFeedback) {
          onFeedback(response.feedback);
        }
      }
    } catch (err) {
      console.error("Failed to submit feedback:", err);
    } finally {
      setSubmitting(false);
    }
  };

  const iconSize = size === "sm" ? "w-4 h-4" : "w-5 h-5";
  const buttonPadding = size === "sm" ? "p-1.5" : "p-2";

  return (
    <div className="flex items-center gap-1">
      <span className="text-xs text-hone-500 dark:text-hone-400 mr-1">
        Was this helpful?
      </span>
      <button
        onClick={() => handleFeedback(true)}
        disabled={submitting || currentState === "helpful"}
        className={`${buttonPadding} rounded transition-colors ${
          currentState === "helpful"
            ? "bg-savings/20 text-savings-dark"
            : "text-hone-400 hover:text-savings-dark hover:bg-savings/10"
        } disabled:opacity-50`}
        title="Yes, this was helpful"
      >
        <ThumbsUp className={iconSize} />
      </button>
      <button
        onClick={() => handleFeedback(false)}
        disabled={submitting || currentState === "not_helpful"}
        className={`${buttonPadding} rounded transition-colors ${
          currentState === "not_helpful"
            ? "bg-waste/20 text-waste"
            : "text-hone-400 hover:text-waste hover:bg-waste/10"
        } disabled:opacity-50`}
        title="No, this wasn't helpful"
      >
        <ThumbsDown className={iconSize} />
      </button>
    </div>
  );
}
