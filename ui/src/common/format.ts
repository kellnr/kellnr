/**
 * Format a count for display, with digit grouping from the browser's locale.
 * Strings are parsed first; values that are not numbers are returned as is.
 */
export function formatNumber(value: number | string, locale?: string): string {
  const num = typeof value === "number" ? value : Number(value);
  if (typeof value === "string" && (value.trim() === "" || Number.isNaN(num))) {
    return value;
  }
  return num.toLocaleString(locale);
}
