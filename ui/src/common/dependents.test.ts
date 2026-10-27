import { test } from "node:test";
import assert from "node:assert/strict";
import { dependentsSummary, sortDependents } from "./dependents.ts";
import type { CrateDependent } from "../types/crate_data";

function dependent(name: string, outdated: boolean): CrateDependent {
  return { name, version: "1.0.0", requirements: [], outdated };
}

test("summarizes dependents that are behind", () => {
  assert.equal(
    dependentsSummary({
      latest_version: "2.0.0",
      dependents: [dependent("a", true), dependent("b", true), dependent("c", false)],
      restricted: 2,
    }),
    "Used by 5 crates, 2 are behind the latest version 2.0.0. Restricted crates are not checked.",
  );
  assert.equal(
    dependentsSummary({ latest_version: "2.0.0", dependents: [dependent("a", true)], restricted: 0 }),
    "Used by 1 crate, 1 is behind the latest version 2.0.0.",
  );
});

test("summarizes dependents that are up to date", () => {
  assert.equal(
    dependentsSummary({ latest_version: "2.0.0", dependents: [dependent("a", false)], restricted: 1 }),
    "Used by 2 crates, none is behind the latest version 2.0.0. Restricted crates are not checked.",
  );
});

test("summarizes dependents without restricted ones", () => {
  assert.equal(
    dependentsSummary({ latest_version: "2.0.0", dependents: [dependent("a", false)], restricted: 0 }),
    "Used by 1 crate, none is behind the latest version 2.0.0.",
  );
});

test("summarizes dependents without a latest version", () => {
  assert.equal(
    dependentsSummary({ dependents: [dependent("a", false)], restricted: 0 }),
    "Used by 1 crate.",
  );
});

test("summarizes a crate without dependents", () => {
  assert.equal(
    dependentsSummary({ latest_version: "1.0.0", dependents: [], restricted: 0 }),
    "No crate in this registry depends on this crate.",
  );
});

test("sorts dependents that are behind first", () => {
  const sorted = sortDependents([
    dependent("b", false),
    dependent("c", true),
    dependent("a", false),
    dependent("d", true),
  ]);
  assert.deepEqual(sorted.map(d => d.name), ["c", "d", "a", "b"]);
});
