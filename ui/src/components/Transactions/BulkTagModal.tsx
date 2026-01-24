import { Check, ChevronRight, X } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api";
import type { TagWithPath } from "../../types";

interface BulkTagModalProps {
  transactionIds: number[];
  action: "add" | "remove";
  onClose: () => void;
  onSuccess: () => void;
}

interface TagCheckNodeProps {
  tag: TagWithPath;
  selectedTagIds: Set<number>;
  expandedTagIds: Set<number>;
  onToggleTag: (id: number) => void;
  onToggleExpand: (id: number) => void;
  depth?: number;
  inheritedColor?: string;
}

function TagCheckNode({
  tag,
  selectedTagIds,
  expandedTagIds,
  onToggleTag,
  onToggleExpand,
  depth = 0,
  inheritedColor,
}: TagCheckNodeProps) {
  const children = tag.children ?? [];
  const hasChildren = children.length > 0;
  const isExpanded = expandedTagIds.has(tag.id);
  const isSelected = selectedTagIds.has(tag.id);
  const effectiveColor = tag.color || inheritedColor;

  return (
    <div>
      <div
        className="flex items-center gap-2 py-1.5 px-2 rounded cursor-pointer hover:bg-hone-100 dark:hover:bg-hone-700"
        style={{ paddingLeft: `${depth * 16 + 8}px` }}
        onClick={() => onToggleTag(tag.id)}
      >
        {hasChildren ? (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onToggleExpand(tag.id);
            }}
            className="p-0.5 hover:bg-hone-200 dark:hover:bg-hone-600 rounded"
          >
            <ChevronRight
              className={`w-4 h-4 text-hone-400 transition-transform ${
                isExpanded ? "rotate-90" : ""
              }`}
            />
          </button>
        ) : (
          <span className="w-5" />
        )}
        <div
          className={`w-4 h-4 rounded border flex items-center justify-center flex-shrink-0 ${
            isSelected
              ? "bg-hone-600 border-hone-600"
              : "border-hone-300 dark:border-hone-500"
          }`}
        >
          {isSelected && <Check className="w-3 h-3 text-white" />}
        </div>
        {effectiveColor && (
          <span
            className="w-2.5 h-2.5 rounded-full flex-shrink-0"
            style={{ backgroundColor: effectiveColor }}
          />
        )}
        <span className="flex-1 truncate text-sm">{tag.name}</span>
      </div>
      {hasChildren && isExpanded && (
        <div>
          {children.map((child) => (
            <TagCheckNode
              key={child.id}
              tag={child}
              selectedTagIds={selectedTagIds}
              expandedTagIds={expandedTagIds}
              onToggleTag={onToggleTag}
              onToggleExpand={onToggleExpand}
              depth={depth + 1}
              inheritedColor={effectiveColor}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export function BulkTagModal({ transactionIds, action, onClose, onSuccess }: BulkTagModalProps) {
  const [tags, setTags] = useState<TagWithPath[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedTagIds, setSelectedTagIds] = useState<Set<number>>(new Set());
  const [expandedTagIds, setExpandedTagIds] = useState<Set<number>>(new Set());

  useEffect(() => {
    api.getTagsTree()
      .then((data) => {
        setTags(data);
        // Expand root tags by default
        setExpandedTagIds(new Set(data.map((t) => t.id)));
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  // Close on Escape
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const toggleTag = (id: number) => {
    setSelectedTagIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const toggleExpand = (id: number) => {
    setExpandedTagIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const handleApply = async () => {
    if (selectedTagIds.size === 0) return;

    try {
      setSaving(true);
      setError(null);

      const tagIds = Array.from(selectedTagIds);
      if (action === "add") {
        await api.bulkAddTags(transactionIds, tagIds);
      } else {
        await api.bulkRemoveTags(transactionIds, tagIds);
      }

      onSuccess();
    } catch (err) {
      setError(err instanceof Error ? err.message : `Failed to ${action} tags`);
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="card w-full max-w-md mx-4 max-h-[80vh] flex flex-col animate-slide-up">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold">
            {action === "add" ? "Add Tags" : "Remove Tags"} ({transactionIds.length} transactions)
          </h2>
          <button onClick={onClose} className="p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200">
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="card-body flex-1 overflow-y-auto min-h-0">
          {loading ? (
            <div className="text-center py-8 text-hone-500">Loading tags...</div>
          ) : tags.length === 0 ? (
            <div className="text-center py-8 text-hone-500">No tags found</div>
          ) : (
            <div className="space-y-0.5">
              {tags.map((tag) => (
                <TagCheckNode
                  key={tag.id}
                  tag={tag}
                  selectedTagIds={selectedTagIds}
                  expandedTagIds={expandedTagIds}
                  onToggleTag={toggleTag}
                  onToggleExpand={toggleExpand}
                />
              ))}
            </div>
          )}
          {error && (
            <div className="mt-4 text-sm text-waste">{error}</div>
          )}
        </div>

        <div className="card-body border-t border-hone-100 dark:border-hone-700 flex items-center justify-between">
          <span className="text-sm text-hone-500">
            {selectedTagIds.size} tag{selectedTagIds.size !== 1 ? "s" : ""} selected
          </span>
          <div className="flex gap-2">
            <button type="button" onClick={onClose} className="btn-secondary">
              Cancel
            </button>
            <button
              onClick={handleApply}
              disabled={selectedTagIds.size === 0 || saving}
              className="btn-primary disabled:opacity-50"
            >
              {saving ? "Applying..." : action === "add" ? "Add Tags" : "Remove Tags"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
