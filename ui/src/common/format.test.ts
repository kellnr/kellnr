import { test } from "node:test";
import assert from "node:assert/strict";
import { formatNumber } from "./format.ts";

test("groups digits by locale", () => {
  assert.equal(formatNumber(10779, "en-US"), "10,779");
  assert.equal(formatNumber(10779, "de-DE"), "10.779");
  assert.equal(formatNumber(5, "en-US"), "5");
});

test("parses numeric strings", () => {
  assert.equal(formatNumber("14040", "en-US"), "14,040");
});

test("returns non-numeric strings unchanged", () => {
  assert.equal(formatNumber("n/a", "en-US"), "n/a");
  assert.equal(formatNumber("", "en-US"), "");
});
