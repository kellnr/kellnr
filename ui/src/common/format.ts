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

const BYTE_UNITS = ["B", "kB", "MB", "GB"];

/**
 * Format a size in bytes with decimal units like crates.io, e.g. "12.3 kB".
 */
export function formatBytes(bytes: number, locale?: string): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < BYTE_UNITS.length - 1) {
    value /= 1000;
    unit++;
  }
  const digits = unit === 0 || value >= 100 ? 0 : 1;
  const number = value.toLocaleString(locale, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
  return `${number} ${BYTE_UNITS[unit]}`;
}

/**
 * Label of the semver compatibility range a version belongs to, like "1.x",
 * "0.3.x" or "0.0.4". Versions with the same label are compatible with each
 * other.
 */
export function versionGroup(version: string): string {
  const [major = "0", minor = "0", patch = "0"] = version.split(/[.+-]/);
  if (major !== "0") return `${major}.x`;
  if (minor !== "0") return `0.${minor}.x`;
  return `0.0.${patch}`;
}

/**
 * A count with its noun, like "1 user" or "3 users".
 */
export function formatCount(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}
