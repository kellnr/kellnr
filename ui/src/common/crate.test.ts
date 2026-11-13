import { test } from "node:test";
import assert from "node:assert/strict";
import { compareVersions, crateLoadOutcome, findVersion, versionSelectItems } from "./crate.ts";
import type { CrateData, CrateVersionData } from "../types/crate_data";

function version(v: string, yanked = false): CrateVersionData {
  return {
    version: v, created: "", downloads: 0, dependencies: [], checksum: "", features: {},
    yanked, v: 1, size: 0,
  };
}

function crate(versions: CrateVersionData[]): CrateData {
  return {
    name: "foo", owners: [], max_version: versions[0]?.version ?? "", total_downloads: 0,
    last_updated: "", categories: [], keywords: [], authors: [], versions,
  };
}

test("compares versions by semver precedence", () => {
  assert.ok(compareVersions("1.10.0", "1.9.0") > 0);
  assert.ok(compareVersions("0.2.0", "0.10.0") < 0);
  assert.ok(compareVersions("1.0.0", "1.0.0-rc.1") > 0);
  assert.ok(compareVersions("1.0.0-alpha.2", "1.0.0-alpha.10") < 0);
  assert.ok(compareVersions("1.0.0-alpha", "1.0.0-beta") < 0);
  assert.ok(compareVersions("1.0.0-alpha", "1.0.0-alpha.1") < 0);
  assert.ok(compareVersions("1.0.0-1", "1.0.0-alpha") < 0);
  assert.equal(compareVersions("1.0.0+build.1", "1.0.0"), 0);
});

test("lists versions newest first and marks yanked ones", () => {
  const items = versionSelectItems([version("0.9.0"), version("1.0.0-rc.1", true), version("1.0.0"), version("0.10.0")]);
  assert.deepEqual(items, [
    { title: "1.0.0", value: "1.0.0", yanked: false },
    { title: "1.0.0-rc.1 (yanked)", value: "1.0.0-rc.1", yanked: true },
    { title: "0.10.0", value: "0.10.0", yanked: false },
    { title: "0.9.0", value: "0.9.0", yanked: false },
  ]);
});

test("finds the requested version or the max version", () => {
  const data = crate([version("1.0.0"), version("0.9.0")]);
  assert.equal(findVersion(data, "0.9.0")?.version, "0.9.0");
  assert.equal(findVersion(data, undefined)?.version, "1.0.0");
  assert.equal(findVersion(data, "2.0.0"), undefined);
});

test("a crate is found when the request succeeds with versions", () => {
  const data = crate([version("1.0.0")]);
  assert.deepEqual(crateLoadOutcome({ data, error: null }), { kind: "found", crate: data });
});

test("a 404 or an empty answer means the crate does not exist", () => {
  assert.deepEqual(
    crateLoadOutcome({ data: null, error: { status: 404, message: "Resource not found." } }),
    { kind: "not-found" },
  );
  assert.deepEqual(crateLoadOutcome({ data: crate([]), error: null }), { kind: "not-found" });
});

test("other failures are load errors", () => {
  assert.deepEqual(
    crateLoadOutcome({ data: null, error: { status: 500, message: "Server error." } }),
    { kind: "error", message: "Server error." },
  );
});
