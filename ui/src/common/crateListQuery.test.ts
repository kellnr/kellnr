import { test } from "node:test";
import assert from "node:assert/strict";
import { buildCrateListQuery, parseCrateSort } from "./crateListQuery.ts";

test("parses known sort orders", () => {
  assert.equal(parseCrateSort("name"), "name");
  assert.equal(parseCrateSort("updated"), "updated");
  assert.equal(parseCrateSort("downloads"), "downloads");
});

test("rejects unknown or missing sort orders", () => {
  assert.equal(parseCrateSort("stars"), undefined);
  assert.equal(parseCrateSort(undefined), undefined);
  assert.equal(parseCrateSort(null), undefined);
  assert.equal(parseCrateSort(["updated"]), undefined);
});

test("an unfiltered list has no query parameters", () => {
  assert.deepEqual(buildCrateListQuery({ search: "", sort: "name", tags: {} }), {});
});

test("keeps the search, a non-default sort order and tags", () => {
  assert.deepEqual(
    buildCrateListQuery({ search: "serde", sort: "downloads", tags: {} }),
    { search: "serde", sort: "downloads" },
  );
  assert.deepEqual(
    buildCrateListQuery({ search: "", sort: "updated", tags: { keyword: "cli" } }),
    { sort: "updated", keyword: "cli" },
  );
});
