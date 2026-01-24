import type { TransactionTag } from "../../types";

interface TagBadgeProps {
  tag: TransactionTag;
  size?: "sm" | "md";
}

export function TagBadge({ tag, size = "md" }: TagBadgeProps) {
  const bgColor = tag.tag_color || "#6b7280";
  const sizeClasses = size === "sm" ? "px-1.5 py-0 text-[10px]" : "px-2 py-0.5 text-xs";

  return (
    <span
      className={`inline-flex items-center rounded font-medium ${sizeClasses}`}
      style={{
        backgroundColor: bgColor + "20",
        color: bgColor,
        border: `1px solid ${bgColor}40`,
      }}
      title={`${tag.tag_path} (${tag.source}${tag.confidence ? ` ${Math.round(tag.confidence * 100)}%` : ""})`}
    >
      {tag.tag_name}
    </span>
  );
}
