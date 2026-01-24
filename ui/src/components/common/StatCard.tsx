import type { ReactNode } from "react";

interface StatCardProps {
  icon: ReactNode;
  label: string;
  value: string;
  subtext?: string;
  highlight?: boolean;
  attention?: boolean;
  onClick?: () => void;
}

export function StatCard({
  icon,
  label,
  value,
  subtext,
  highlight,
  attention,
  onClick,
}: StatCardProps) {
  const baseClasses = `stat-card hover-lift ${highlight ? "ring-2 ring-savings/20" : ""} ${attention ? "ring-2 ring-attention/20" : ""}`;
  const clickableClasses = onClick ? "cursor-pointer" : "";

  return (
    <div className={`${baseClasses} ${clickableClasses}`} onClick={onClick}>
      <div className={`${highlight ? "text-savings" : attention ? "text-attention" : "text-hone-400"}`}>{icon}</div>
      <div className="stat-label">{label}</div>
      <div className={`stat-value ${highlight ? "text-savings" : attention ? "text-attention" : ""}`}>{value}</div>
      {subtext && <div className="text-sm text-hone-400 mt-1">{subtext}</div>}
    </div>
  );
}
