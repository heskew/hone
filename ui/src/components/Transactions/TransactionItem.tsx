import { Smartphone, Square, SquareCheck } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api";
import type { Transaction, TransactionTag } from "../../types";
import { TagBadge } from "../common";
import { SplitsModal } from "./SplitsModal";

interface TransactionItemProps {
  transaction: Transaction;
  onArchive?: () => void;
  selectionMode?: boolean;
  selected?: boolean;
  onToggleSelect?: (id: number) => void;
}

export function TransactionItem({ transaction, onArchive, selectionMode, selected, onToggleSelect }: TransactionItemProps) {
  const [tags, setTags] = useState<TransactionTag[]>(transaction.tags || []);
  const [loadingTags, setLoadingTags] = useState(!transaction.tags);
  const [showSplits, setShowSplits] = useState(false);

  useEffect(() => {
    if (!transaction.tags) {
      loadTags();
    }
  }, [transaction.id]);

  const loadTags = async () => {
    try {
      const result = await api.getTransactionTags(transaction.id);
      setTags(result);
    } catch (err) {
      console.error("Failed to load tags:", err);
    } finally {
      setLoadingTags(false);
    }
  };

  const isExpense = transaction.amount < 0;
  // Parse date as local time to avoid timezone shift (YYYY-MM-DD at noon local)
  const [year, month, day] = transaction.date.split("-").map(Number);
  const localDate = new Date(year, month - 1, day, 12, 0, 0);
  const formattedDate = localDate.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "2-digit",
  });

  // Prefer normalized merchant name when available
  const displayName = transaction.merchant_normalized || transaction.description;

  // Use card_member column directly (stored during import)
  const cardMember = transaction.card_member;

  return (
    <>
      {/* Desktop: compact row layout */}
      <div
        className="hidden md:flex items-center gap-4 px-4 py-2.5 hover:bg-hone-100 dark:hover:bg-hone-700 transition-colors cursor-pointer"
        onClick={() => setShowSplits(true)}
      >
        {/* Selection checkbox */}
        {selectionMode && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onToggleSelect?.(transaction.id);
            }}
            className="flex-shrink-0 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200"
          >
            {selected ? (
              <SquareCheck className="w-5 h-5 text-hone-600 dark:text-hone-300" />
            ) : (
              <Square className="w-5 h-5" />
            )}
          </button>
        )}
        {/* Date */}
        <div className="w-20 text-sm text-hone-400 flex-shrink-0">
          {formattedDate}
        </div>

        {/* Merchant / Description */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="truncate font-medium" title={transaction.description}>
              {displayName}
            </span>
            {transaction.payment_method === "apple_pay" && (
              <span title="Apple Pay">
                <Smartphone className="w-3.5 h-3.5 text-hone-400 flex-shrink-0" />
              </span>
            )}
          </div>
          {cardMember && (
            <div className="text-xs text-hone-400 truncate">{cardMember}</div>
          )}
        </div>

        {/* Tags */}
        <div className="flex items-center gap-1 flex-shrink-0 max-w-[200px]">
          {loadingTags ? (
            <span className="text-xs text-hone-400">...</span>
          ) : tags.length > 0 ? (
            tags.slice(0, 2).map((tag) => (
              <TagBadge key={tag.tag_id} tag={tag} size="sm" />
            ))
          ) : (
            <span className="text-xs text-hone-400 italic">—</span>
          )}
          {tags.length > 2 && (
            <span className="text-xs text-hone-400">+{tags.length - 2}</span>
          )}
        </div>

        {/* Amount */}
        <div className={`w-24 text-right font-semibold flex-shrink-0 ${isExpense ? "amount-negative" : "amount-positive"}`}>
          {isExpense ? "-" : "+"}${Math.abs(transaction.amount).toFixed(2)}
        </div>
      </div>

      {/* Mobile: stacked card layout */}
      <div
        className="md:hidden p-4 hover:bg-hone-100 dark:hover:bg-hone-700 transition-colors cursor-pointer"
        onClick={() => setShowSplits(true)}
      >
        <div className="flex items-start justify-between gap-4">
          {/* Selection checkbox */}
          {selectionMode && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onToggleSelect?.(transaction.id);
              }}
              className="flex-shrink-0 mt-0.5 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200"
            >
              {selected ? (
                <SquareCheck className="w-5 h-5 text-hone-600 dark:text-hone-300" />
              ) : (
                <Square className="w-5 h-5" />
              )}
            </button>
          )}
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-1.5">
              <span className="font-medium truncate" title={transaction.description}>{displayName}</span>
              {transaction.payment_method === "apple_pay" && (
                <span title="Apple Pay">
                  <Smartphone className="w-3.5 h-3.5 text-hone-400 flex-shrink-0" />
                </span>
              )}
            </div>
            <div className="text-sm text-hone-400">
              {(() => {
                const [y, m, d] = transaction.date.split("-").map(Number);
                return new Date(y, m - 1, d, 12, 0, 0).toLocaleDateString("en-US", {
                  month: "short",
                  day: "numeric",
                  year: "numeric",
                });
              })()}
              {cardMember && <span className="ml-2">• {cardMember}</span>}
            </div>
            {/* Tags */}
            <div className="flex flex-wrap items-center gap-1 mt-2">
              {loadingTags ? (
                <span className="text-xs text-hone-400">Loading tags...</span>
              ) : tags.length > 0 ? (
                tags.map((tag) => (
                  <TagBadge key={tag.tag_id} tag={tag} />
                ))
              ) : (
                <span className="text-xs text-hone-400 italic">No tags</span>
              )}
            </div>
          </div>
          <div className={`font-semibold whitespace-nowrap ${isExpense ? "amount-negative" : "amount-positive"}`}>
            {isExpense ? "-" : "+"}${Math.abs(transaction.amount).toFixed(2)}
          </div>
        </div>
      </div>

      {showSplits && (
        <SplitsModal
          transaction={transaction}
          onClose={() => setShowSplits(false)}
          onArchive={onArchive}
          onTagsChange={loadTags}
        />
      )}
    </>
  );
}
