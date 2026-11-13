import type { ApiResult } from "../types/api";
import type { CrateData, CrateVersionData } from "../types/crate_data";

export type CrateLoadOutcome =
  | { kind: "found"; crate: CrateData }
  | { kind: "not-found" }
  | { kind: "error"; message: string };

export function crateLoadOutcome(result: ApiResult<CrateData>): CrateLoadOutcome {
  if (result.error !== null) {
    return result.error.status === 404
      ? { kind: "not-found" }
      : { kind: "error", message: result.error.message };
  }
  if (result.data === null || result.data.versions.length === 0) {
    return { kind: "not-found" };
  }
  return { kind: "found", crate: result.data };
}

/** The requested version, or the max version if none is requested */
export function findVersion(crate: CrateData, requested: string | undefined): CrateVersionData | undefined {
  const wanted = requested ?? crate.max_version;
  return crate.versions.find((v) => v.version === wanted);
}

type ParsedVersion = { core: number[]; pre: string[] };

function parseVersion(version: string): ParsedVersion {
  const withoutBuild = version.split("+")[0];
  const dash = withoutBuild.indexOf("-");
  const core = dash === -1 ? withoutBuild : withoutBuild.slice(0, dash);
  const pre = dash === -1 ? "" : withoutBuild.slice(dash + 1);
  return {
    core: core.split(".").map((part) => Number.parseInt(part, 10) || 0),
    pre: pre === "" ? [] : pre.split("."),
  };
}

const NUMERIC = /^\d+$/;

function comparePreIdentifier(a: string, b: string): number {
  const aNumeric = NUMERIC.test(a);
  const bNumeric = NUMERIC.test(b);
  if (aNumeric && bNumeric) return Number(a) - Number(b);
  if (aNumeric) return -1;
  if (bNumeric) return 1;
  return a < b ? -1 : a > b ? 1 : 0;
}

function comparePrerelease(a: string[], b: string[]): number {
  // A version without a pre-release is newer than one with it
  if (a.length === 0 || b.length === 0) return b.length - a.length;
  for (let i = 0; i < Math.min(a.length, b.length); i++) {
    const order = comparePreIdentifier(a[i], b[i]);
    if (order !== 0) return order;
  }
  return a.length - b.length;
}

/** Semver precedence, build metadata is ignored */
export function compareVersions(a: string, b: string): number {
  const left = parseVersion(a);
  const right = parseVersion(b);
  for (let i = 0; i < 3; i++) {
    const order = (left.core[i] ?? 0) - (right.core[i] ?? 0);
    if (order !== 0) return order;
  }
  return comparePrerelease(left.pre, right.pre);
}

export type VersionSelectItem = { title: string; value: string; yanked: boolean };

export function versionSelectItems(versions: CrateVersionData[]): VersionSelectItem[] {
  return [...versions]
    .sort((a, b) => compareVersions(b.version, a.version))
    .map((v) => ({
      title: v.yanked ? `${v.version} (yanked)` : v.version,
      value: v.version,
      yanked: v.yanked,
    }));
}
