import { Plus, ArrowRight } from "lucide-react";
import type { TagWithPath } from "../../types";

interface TagTreeNodeProps {
  tag: TagWithPath;
  selectedId?: number;
  expandedTags: Set<number>;
  onSelect: (tag: TagWithPath) => void;
  onToggleExpand: (id: number) => void;
  onCreateChild: (parentId: number) => void;
  depth?: number;
}

export function TagTreeNode({
  tag,
  selectedId,
  expandedTags,
  onSelect,
  onToggleExpand,
  onCreateChild,
  depth = 0,
}: TagTreeNodeProps) {
  const children = tag.children ?? [];
  const hasChildren = children.length > 0;
  const isExpanded = expandedTags.has(tag.id);
  const isSelected = selectedId === tag.id;

  return (
    <div>
      <div
        className={`flex items-center gap-2 py-1.5 px-2 rounded cursor-pointer hover:bg-hone-100 dark:hover:bg-hone-700 ${
          isSelected ? "bg-hone-100 dark:bg-hone-700" : ""
        }`}
        style={{ paddingLeft: `${depth * 20 + 8}px` }}
        onClick={() => onSelect(tag)}
      >
        {hasChildren ? (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onToggleExpand(tag.id);
            }}
            className="p-0.5 hover:bg-hone-200 rounded"
          >
            <ArrowRight
              className={`w-4 h-4 text-hone-400 transition-transform ${
                isExpanded ? "rotate-90" : ""
              }`}
            />
          </button>
        ) : (
          <span className="w-5" />
        )}
        {tag.color && (
          <span
            className="w-3 h-3 rounded-full flex-shrink-0"
            style={{ backgroundColor: tag.color }}
          />
        )}
        <span className="flex-1 truncate">{tag.name}</span>
        {hasChildren && (
          <span className="text-xs text-hone-400">({children.length})</span>
        )}
        <button
          onClick={(e) => {
            e.stopPropagation();
            onCreateChild(tag.id);
          }}
          className="p-0.5 text-hone-400 hover:text-hone-600 opacity-0 group-hover:opacity-100 hover:bg-hone-200 rounded"
          title="Add child tag"
        >
          <Plus className="w-3 h-3" />
        </button>
      </div>
      {hasChildren && isExpanded && (
        <div>
          {children.map((child) => (
            <TagTreeNode
              key={child.id}
              tag={child}
              selectedId={selectedId}
              expandedTags={expandedTags}
              onSelect={onSelect}
              onToggleExpand={onToggleExpand}
              onCreateChild={onCreateChild}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  );
}
