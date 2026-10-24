import { test } from "node:test";
import assert from "node:assert/strict";
import { isSafeUrl, safeUrl } from "./url.ts";

test("allows http and https URLs", () => {
  assert.equal(isSafeUrl("https://github.com/kellnr/kellnr"), true);
  assert.equal(isSafeUrl("http://example.com"), true);
  assert.equal(isSafeUrl("HTTPS://EXAMPLE.COM"), true);
});

test("allows relative paths of hosted docs", () => {
  assert.equal(isSafeUrl("/docs/my-crate/1.0.0/doc/my_crate/index.html"), true);
  assert.equal(isSafeUrl("/kellnr/docs/my-crate/1.0.0/doc/my_crate/index.html"), true);
  assert.equal(isSafeUrl("kellnr/docs/my-crate/1.0.0/doc/my_crate/index.html"), true);
});

test("rejects script and data URLs", () => {
  assert.equal(isSafeUrl("javascript:alert(1)"), false);
  assert.equal(isSafeUrl("JavaScript:alert(1)"), false);
  assert.equal(isSafeUrl(" javascript:alert(1)"), false);
  assert.equal(isSafeUrl("java\nscript:alert(1)"), false);
  assert.equal(isSafeUrl("\tjavascript:alert(1)"), false);
  assert.equal(isSafeUrl("data:text/html,<script>alert(1)</script>"), false);
  assert.equal(isSafeUrl("vbscript:msgbox(1)"), false);
  assert.equal(isSafeUrl("file:///etc/passwd"), false);
});

test("rejects empty values", () => {
  assert.equal(isSafeUrl(""), false);
  assert.equal(isSafeUrl(null), false);
  assert.equal(isSafeUrl(undefined), false);
});

test("safeUrl returns the URL only if it is safe", () => {
  assert.equal(safeUrl("https://example.com"), "https://example.com");
  assert.equal(safeUrl("javascript:alert(1)"), undefined);
  assert.equal(safeUrl(null), undefined);
});
