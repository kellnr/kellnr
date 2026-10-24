// Base to resolve relative URLs against. Relative URLs inherit the protocol
// of the page, which is always http or https, so any such base works.
const RELATIVE_URL_BASE = "http://localhost/";

const SAFE_PROTOCOLS = ["http:", "https:"];

/**
 * Whether a URL from crate metadata is safe to use as a link target.
 * Only http(s) URLs and relative paths (like the hosted docs links kellnr
 * generates) are allowed, which rules out `javascript:` and `data:` URLs.
 */
export function isSafeUrl(url: string | null | undefined): url is string {
  if (!url) {
    return false;
  }
  try {
    return SAFE_PROTOCOLS.includes(new URL(url, RELATIVE_URL_BASE).protocol);
  } catch {
    return false;
  }
}

/**
 * Returns the URL if it is safe to use as a link target, otherwise `undefined`.
 */
export function safeUrl(url: string | null | undefined): string | undefined {
  return isSafeUrl(url) ? url : undefined;
}
