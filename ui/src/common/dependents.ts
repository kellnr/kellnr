import type { CrateDependent, CrateDependents } from "../types/crate_data";

function crates(count: number): string {
  return count === 1 ? "1 crate" : `${count} crates`;
}

/** Summary line of the "Used by" tab, e.g. "Used by 5 crates, 3 are behind the latest version 2.0.0." */
export function dependentsSummary(dependents: CrateDependents): string {
  const total = dependents.dependents.length + dependents.restricted;
  if (total === 0) {
    return "No crate in this registry depends on this crate.";
  }
  if (!dependents.latest_version) {
    return `Used by ${crates(total)}.`;
  }
  const behind = dependents.dependents.filter(d => d.outdated).length;
  const latest = `the latest version ${dependents.latest_version}`;
  const summary = behind === 0
    ? `Used by ${crates(total)}, none is behind ${latest}.`
    : `Used by ${crates(total)}, ${behind} ${behind === 1 ? "is" : "are"} behind ${latest}.`;
  // Nothing is known about restricted dependents, not even if they are behind
  return dependents.restricted > 0
    ? `${summary} Restricted crates are not checked.`
    : summary;
}

/** Dependents that are behind first, then by name */
export function sortDependents(dependents: CrateDependent[]): CrateDependent[] {
  return [...dependents].sort((a, b) =>
    Number(b.outdated) - Number(a.outdated) || a.name.localeCompare(b.name));
}
