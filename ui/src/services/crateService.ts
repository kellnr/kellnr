/**
 * Crate API service
 */
import { apiGet, apiPost, apiPut, apiDelete } from './api'
import type { ApiResult } from '../types/api'
import type {
  CrateSort,
  CrateTagFilter,
  CratesResponse,
  SearchResponse,
  CrateUsersResponse,
  CrateGroupsResponse,
  CrateOwnersResponse,
  CrateAccessDataResponse,
  CrateAccessDataRequest,
} from '../types/api'
import type { CrateData, CrateDependents, CratesIoCrateData } from '../types/crate_data'
import type { Statistics } from '../types/statistics'
import {
  CRATES,
  SEARCH,
  CRATE_DATA,
  CRATE_DEPENDENTS,
  CRATESIO_DATA,
  CRATE_DELETE,
  CRATE_DELETE_VERSION,
  STATISTICS,
  CRATE_ACL,
  CRATE_OWNERS,
  CRATE_OWNER,
  CRATE_USERS,
  CRATE_USER,
  CRATE_GROUPS,
  CRATE_GROUP,
  DOCS_BUILDS,
} from '../remote-routes'
import type { DocQueueItem } from '../types/doc_queue_item'

/**
 * Get paginated list of crates
 */
export async function getCrates(
  page: number,
  pageSize: number,
  cache: boolean,
  sort: CrateSort = 'name',
  tags: CrateTagFilter = {}
): Promise<ApiResult<CratesResponse>> {
  return apiGet<CratesResponse>(CRATES, { page, page_size: pageSize, cache, sort, ...tags })
}

/**
 * Get paginated search results for crates by name
 */
export async function searchCrates(
  name: string,
  page: number,
  pageSize: number,
  cache: boolean
): Promise<ApiResult<SearchResponse>> {
  return apiGet<SearchResponse>(SEARCH, { name, page, page_size: pageSize, cache })
}

/**
 * Get detailed crate data
 */
export async function getCrateData(name: string): Promise<ApiResult<CrateData>> {
  return apiGet<CrateData>(CRATE_DATA, { name })
}

/**
 * Get the crates whose latest version depends on a crate
 */
export async function getCrateDependents(name: string): Promise<ApiResult<CrateDependents>> {
  return apiGet<CrateDependents>(CRATE_DEPENDENTS, { name })
}

/**
 * Successful crates.io responses, kept for the lifetime of the page so the
 * dependency list does not query crates.io again on every tab switch.
 */
const cratesIoDataCache = new Map<string, Promise<ApiResult<CratesIoCrateData>>>()

/**
 * Get crates.io crate data (for cached crates)
 */
export async function getCratesIoData(name: string): Promise<ApiResult<CratesIoCrateData>> {
  const cached = cratesIoDataCache.get(name)
  if (cached) {
    return cached
  }

  const request = apiGet<CratesIoCrateData>(CRATESIO_DATA, { name })
  cratesIoDataCache.set(name, request)
  const result = await request
  if (result.error) {
    cratesIoDataCache.delete(name)
  }
  return result
}

/**
 * Delete a specific version of a crate
 */
export async function deleteCrateVersion(
  name: string,
  version: string
): Promise<ApiResult<void>> {
  return apiDelete<void>(CRATE_DELETE_VERSION(name, version))
}

// Alias for consistency
export const deleteVersion = deleteCrateVersion

/**
 * Delete all versions of a crate
 */
export async function deleteCrate(name: string): Promise<ApiResult<void>> {
  return apiDelete<void>(CRATE_DELETE(name))
}

/**
 * Get registry statistics
 */
export async function getStatistics(): Promise<ApiResult<Statistics>> {
  return apiGet<Statistics>(STATISTICS)
}

// --- Crate Access Control ---

/**
 * Get users with access to a crate
 */
export async function getCrateUsers(crateName: string): Promise<ApiResult<CrateUsersResponse>> {
  return apiGet<CrateUsersResponse>(CRATE_USERS(crateName))
}

/**
 * Add a user to a crate
 */
export async function addCrateUser(
  crateName: string,
  userName: string
): Promise<ApiResult<void>> {
  return apiPut<void>(CRATE_USER(crateName, userName), null, {
    customErrors: {
      404: 'User not found. Did you provide an existing user name?',
    },
  })
}

/**
 * Remove a user from a crate
 */
export async function removeCrateUser(
  crateName: string,
  userName: string
): Promise<ApiResult<void>> {
  return apiDelete<void>(CRATE_USER(crateName, userName))
}

// Alias for consistency
export const deleteCrateUser = removeCrateUser

/**
 * Get groups with access to a crate
 */
export async function getCrateGroups(
  crateName: string
): Promise<ApiResult<CrateGroupsResponse>> {
  return apiGet<CrateGroupsResponse>(CRATE_GROUPS(crateName))
}

/**
 * Add a group to a crate
 */
export async function addCrateGroup(
  crateName: string,
  groupName: string
): Promise<ApiResult<void>> {
  return apiPut<void>(CRATE_GROUP(crateName, groupName), null, {
    customErrors: {
      404: 'Group not found. Did you provide an existing group name?',
    },
  })
}

/**
 * Remove a group from a crate
 */
export async function removeCrateGroup(
  crateName: string,
  groupName: string
): Promise<ApiResult<void>> {
  return apiDelete<void>(CRATE_GROUP(crateName, groupName))
}

// Alias for consistency
export const deleteCrateGroup = removeCrateGroup

// --- Crate Owners ---

/**
 * Get owners of a crate
 */
export async function getCrateOwners(
  crateName: string
): Promise<ApiResult<CrateOwnersResponse>> {
  return apiGet<CrateOwnersResponse>(CRATE_OWNERS(crateName))
}

/**
 * Add an owner to a crate
 */
export async function addCrateOwner(
  crateName: string,
  userName: string
): Promise<ApiResult<void>> {
  return apiPut<void>(CRATE_OWNER(crateName, userName), null, {
    customErrors: {
      403: 'Not allowed. Only existing owners or admins can add owners.',
      404: 'User not found. Did you provide an existing user name?',
    },
  })
}

/**
 * Remove an owner from a crate
 */
export async function removeCrateOwner(
  crateName: string,
  userName: string
): Promise<ApiResult<void>> {
  return apiDelete<void>(CRATE_OWNER(crateName, userName), undefined, {
    customErrors: {
      403: 'Not allowed. Only existing owners or admins can remove owners.',
      409: 'A crate must have at least one owner.',
    },
  })
}

// Alias for consistency
export const deleteCrateOwner = removeCrateOwner

// --- Crate Access Data ---

/**
 * Get crate access data (download restrictions)
 */
export async function getCrateAccessData(
  crateName: string
): Promise<ApiResult<CrateAccessDataResponse>> {
  return apiGet<CrateAccessDataResponse>(CRATE_ACL(crateName))
}

/**
 * Set crate access data (download restrictions)
 */
export async function setCrateAccessData(
  crateName: string,
  downloadRestricted: boolean
): Promise<ApiResult<CrateAccessDataResponse>> {
  const data: CrateAccessDataRequest = { download_restricted: downloadRestricted }
  return apiPut<CrateAccessDataResponse>(CRATE_ACL(crateName), data)
}

// --- Documentation ---

/**
 * Request documentation build for a crate version
 */
export async function buildDocs(
  crateName: string,
  version: string
): Promise<ApiResult<void>> {
  return apiPost<void>(DOCS_BUILDS, null, { package: crateName, version })
}

/**
 * Get documentation build queue
 */
export async function getDocsQueue(): Promise<ApiResult<DocQueueItem[]>> {
  // Backend returns { queue: [...] }, we need to extract the array
  const result = await apiGet<{ queue: DocQueueItem[] }>(DOCS_BUILDS)
  if (result.error) {
    return { data: null, error: result.error }
  }
  return { data: result.data?.queue ?? [], error: null }
}

// --- Helper for getting all groups (used in crate settings) ---

/**
 * Get all available groups (re-exported from groupService for convenience)
 */
export { getGroups as getAllGroups } from './groupService'
