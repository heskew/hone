import { X } from "lucide-react";
import { api } from "../../../api";
import type { TagWithPath, Transaction, TransactionTag } from "../../../types";

interface TagsTabProps {
  transaction: Transaction;
  transactionTags: TransactionTag[];
  allTags: TagWithPath[];
  loading: boolean;
  onTagsChange: () => void;
  onError: (error: string | null) => void;
}

export function TagsTab({
  transaction,
  transactionTags,
  allTags,
  loading,
  onTagsChange,
  onError,
}: TagsTabProps) {
  const handleAddTag = async (tagId: number) => {
    try {
      onError(null);
      await api.addTransactionTag(transaction.id, tagId);
      onTagsChange();
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to add tag");
    }
  };

  const handleRemoveTag = async (tagId: number) => {
    try {
      onError(null);
      await api.removeTransactionTag(transaction.id, tagId);
      onTagsChange();
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to remove tag");
    }
  };

  // Flatten tags with depth for indentation
  const flattenTags = (
    tagList: TagWithPath[],
    depth = 0
  ): { tag: TagWithPath; depth: number }[] => {
    const result: { tag: TagWithPath; depth: number }[] = [];
    for (const tag of tagList) {
      result.push({ tag, depth });
      if (tag.children && tag.children.length > 0) {
        result.push(...flattenTags(tag.children, depth + 1));
      }
    }
    return result;
  };

  if (loading) {
    return <p className="text-hone-500 text-center py-4">Loading...</p>;
  }

  const flatTags = flattenTags(allTags);
  const assignedTagIds = new Set(transactionTags.map((t) => t.tag_id));

  return (
    <div className="space-y-4">
      {/* Current tags */}
      <div>
        <h3 className="text-sm font-medium text-hone-700 dark:text-hone-300 mb-2">
          Current Tags
        </h3>
        {transactionTags.length === 0 ? (
          <p className="text-sm text-hone-400 italic">No tags assigned</p>
        ) : (
          <div className="flex flex-wrap gap-2">
            {transactionTags.map((tag) => (
              <div
                key={tag.tag_id}
                className="flex items-center gap-1 px-2 py-1 rounded-full text-sm"
                style={{
                  backgroundColor: tag.tag_color
                    ? `${tag.tag_color}20`
                    : "rgb(var(--color-hone-100))",
                  borderWidth: "1px",
                  borderColor: tag.tag_color || "rgb(var(--color-hone-300))",
                }}
              >
                {tag.tag_color && (
                  <span
                    className="w-2 h-2 rounded-full"
                    style={{ backgroundColor: tag.tag_color }}
                  />
                )}
                <span className="text-hone-700 dark:text-hone-200">
                  {tag.tag_name}
                </span>
                <span className="text-xs text-hone-400">
                  ({tag.source})
                </span>
                <button
                  onClick={() => handleRemoveTag(tag.tag_id)}
                  className="ml-1 text-hone-400 hover:text-waste"
                  title="Remove tag"
                >
                  <X className="w-3 h-3" />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Add tag */}
      <div className="border-t border-hone-100 dark:border-hone-700 pt-4">
        <h3 className="text-sm font-medium text-hone-700 dark:text-hone-300 mb-2">
          Add Tag
        </h3>
        <div className="max-h-60 overflow-y-auto border border-hone-200 dark:border-hone-700 rounded-lg">
          {flatTags.map(({ tag, depth }) => {
            const isAssigned = assignedTagIds.has(tag.id);
            return (
              <button
                key={tag.id}
                onClick={() => !isAssigned && handleAddTag(tag.id)}
                disabled={isAssigned}
                className={`w-full px-3 py-2 text-left text-sm flex items-center gap-2 border-b border-hone-100 dark:border-hone-700 last:border-b-0 transition-colors ${
                  isAssigned
                    ? "bg-hone-50 dark:bg-hone-800/50 text-hone-400 cursor-default"
                    : "hover:bg-hone-50 dark:hover:bg-hone-700/30"
                }`}
                style={{ paddingLeft: `${depth * 16 + 12}px` }}
              >
                {tag.color && (
                  <span
                    className="w-2.5 h-2.5 rounded-full flex-shrink-0"
                    style={{ backgroundColor: tag.color }}
                  />
                )}
                <span className="flex-1">{tag.name}</span>
                {isAssigned && (
                  <span className="text-xs text-hone-400">assigned</span>
                )}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
