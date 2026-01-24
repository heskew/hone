/**
 * Parse a YYYY-MM-DD date string as a local date (not UTC).
 *
 * JavaScript's Date constructor interprets "YYYY-MM-DD" as midnight UTC,
 * which can shift to the previous day when displayed in local time.
 * This function creates a Date at noon local time to avoid that issue.
 */
export function parseLocalDate(dateStr: string): Date {
  const [year, month, day] = dateStr.split("-").map(Number);
  return new Date(year, month - 1, day, 12, 0, 0);
}

/**
 * Format a YYYY-MM-DD date string for display without timezone shift.
 */
export function formatDate(dateStr: string, options?: Intl.DateTimeFormatOptions): string {
  const defaultOptions: Intl.DateTimeFormatOptions = {
    month: "short",
    day: "numeric",
    year: "2-digit",
  };
  return parseLocalDate(dateStr).toLocaleDateString("en-US", options || defaultOptions);
}
