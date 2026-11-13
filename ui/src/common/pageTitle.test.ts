import { test } from "node:test";
import assert from "node:assert/strict";
import { formatPageTitle } from "./pageTitle.ts";

test("appends the app name to a page title", () => {
  assert.equal(formatPageTitle("Search"), "Search - Kellnr");
  assert.equal(formatPageTitle("acme-core 1.0.0"), "acme-core 1.0.0 - Kellnr");
});

test("uses the app name alone without a page title", () => {
  assert.equal(formatPageTitle(undefined), "Kellnr");
  assert.equal(formatPageTitle(""), "Kellnr");
  assert.equal(formatPageTitle("   "), "Kellnr");
});

test("trims the page title", () => {
  assert.equal(formatPageTitle("  Settings "), "Settings - Kellnr");
});
