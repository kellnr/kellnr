import type { CrateSort, CrateTagFilter } from "../types/api";

const CRATE_SORTS: readonly CrateSort[] = ["name", "updated", "downloads"];

/** The default order, left out of the URL */
export const DEFAULT_CRATE_SORT: CrateSort = "name";

export function parseCrateSort(value: unknown): CrateSort | undefined {
  return CRATE_SORTS.find(sort => sort === value);
}

/** `proxy=1` lists the cached crates.io crates, without it the list shows the registry crates */
export function parseCrateProxy(value: unknown): boolean {
  return value === "1";
}

export interface CrateListState {
  search: string
  sort: CrateSort
  tags: CrateTagFilter
  proxy: boolean
}

/** Query parameters of the crate list URL, so a list can be linked and survives a reload */
export function buildCrateListQuery({ search, sort, tags, proxy }: CrateListState): Record<string, string> {
  return {
    ...(proxy ? { proxy: "1" } : {}),
    ...(search ? { search } : {}),
    ...(sort !== DEFAULT_CRATE_SORT ? { sort } : {}),
    ...(tags.keyword ? { keyword: tags.keyword } : {}),
    ...(tags.category ? { category: tags.category } : {}),
  };
}
