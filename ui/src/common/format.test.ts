import { test } from "node:test";
import assert from "node:assert/strict";
import { formatBytes, formatCount, formatNumber, versionGroup } from "./format.ts";

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

test("formats sizes with decimal units", () => {
  assert.equal(formatBytes(512, "en-US"), "512 B");
  assert.equal(formatBytes(12345, "en-US"), "12.3 kB");
  assert.equal(formatBytes(123456, "en-US"), "123 kB");
  assert.equal(formatBytes(4_500_000, "en-US"), "4.5 MB");
});

test("groups versions by semver compatibility", () => {
  assert.equal(versionGroup("1.2.3"), "1.x");
  assert.equal(versionGroup("12.0.0-beta.1"), "12.x");
  assert.equal(versionGroup("0.3.7"), "0.3.x");
  assert.equal(versionGroup("0.0.4"), "0.0.4");
  assert.equal(versionGroup("0.0.4+build"), "0.0.4");
});

test("formats counts with singular and plural nouns", () => {
  assert.equal(formatCount(0, "user"), "0 users");
  assert.equal(formatCount(1, "user"), "1 user");
  assert.equal(formatCount(3, "group"), "3 groups");
});
