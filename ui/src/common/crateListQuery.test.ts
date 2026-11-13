import { test } from "node:test";
import assert from "node:assert/strict";
import { buildCrateListQuery, parseCrateProxy, parseCrateSort } from "./crateListQuery.ts";

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
  assert.deepEqual(buildCrateListQuery({ search: "", sort: "name", tags: {}, proxy: false }), {});
});

test("keeps the search, a non-default sort order and tags", () => {
  assert.deepEqual(
    buildCrateListQuery({ search: "serde", sort: "downloads", tags: {}, proxy: false }),
    { search: "serde", sort: "downloads" },
  );
  assert.deepEqual(
    buildCrateListQuery({ search: "", sort: "updated", tags: { keyword: "cli" }, proxy: false }),
    { sort: "updated", keyword: "cli" },
  );
});

test("only proxy=1 selects the cached crates.io crates", () => {
  assert.equal(parseCrateProxy("1"), true);
  assert.equal(parseCrateProxy("0"), false);
  assert.equal(parseCrateProxy("true"), false);
  assert.equal(parseCrateProxy(""), false);
  assert.equal(parseCrateProxy(undefined), false);
  assert.equal(parseCrateProxy(null), false);
  assert.equal(parseCrateProxy(["1"]), false);
});

test("keeps the proxy mode, registry crates leave it out", () => {
  assert.deepEqual(
    buildCrateListQuery({ search: "", sort: "name", tags: {}, proxy: true }),
    { proxy: "1" },
  );
  assert.deepEqual(
    buildCrateListQuery({ search: "serde", sort: "downloads", tags: {}, proxy: true }),
    { proxy: "1", search: "serde", sort: "downloads" },
  );
});
