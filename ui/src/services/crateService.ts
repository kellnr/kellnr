/**
 * Crate API service
 */
import { apiGet, apiPost, apiPut, apiDelete } from './api'
import type { ApiResult } from '../types/api'
import type {
  CratesResponse,
  SearchResponse,
  CrateUsersResponse,
  CrateGroupsResponse,
  CrateOwnersResponse,
  CrateAccessDataResponse,
  CrateAccessDataRequest,
} from '../types/api'
import type { CrateData } from '../types/crate_data'
import type { Statistics } from '../types/statistics'
import {
  CRATES,
  SEARCH,
  CRATE_DATA,
  CRATESIO_DATA,
  CRATE_DELETE_VERSION,
  CRATE_DELETE_ALL,
  STATISTICS,
  CRATE_USERS,
  CRATE_USER,
  CRATE_GROUPS,
  CRATE_GROUP,
  CRATE_OWNERS,
  CRATE_OWNER,
  CRATE_ACCESS_DATA,
  DOCS_BUILD,
  DOCS_QUEUE,
} from '../remote-routes'
import type { DocQueueItem } from '../types/doc_queue_item'

/**
 * Get paginated list of crates
 */
export async function getCrates(
  page: number,
  pageSize: number,
  cache: boolean
): Promise<ApiResult<CratesResponse>> {
  return apiGet<CratesResponse>(CRATES, { page, page_size: pageSize, cache })
}

/**
 * Search for crates by name
 */
export async function searchCrates(
  name: string,
  cache: boolean
): Promise<ApiResult<SearchResponse>> {
  return apiGet<SearchResponse>(SEARCH, { name, cache })
}

/**
 * Get detailed crate data
 */
export async function getCrateData(name: string): Promise<ApiResult<CrateData>> {
  return apiGet<CrateData>(CRATE_DATA, { name })
}

/**
 * Get crates.io crate data (for cached crates)
 */
export async function getCratesIoData(name: string): Promise<ApiResult<CrateData>> {
  return apiGet<CrateData>(CRATESIO_DATA, { name })
}

/**
 * Delete a specific version of a crate
 */
export async function deleteCrateVersion(
  name: string,
  version: string
): Promise<ApiResult<void>> {
  return apiDelete<void>(CRATE_DELETE_VERSION, { name, version })
}

// Alias for consistency
export const deleteVersion = deleteCrateVersion

/**
 * Delete all versions of a crate
 */
export async function deleteCrate(name: string): Promise<ApiResult<void>> {
  return apiDelete<void>(CRATE_DELETE_ALL, { name })
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
  return apiGet<CrateUsersResponse>(CRATE_USERS(crateName), undefined, { noCache: true })
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
  return apiGet<CrateGroupsResponse>(CRATE_GROUPS(crateName), undefined, { noCache: true })
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
  return apiGet<CrateOwnersResponse>(CRATE_OWNERS(crateName), undefined, { noCache: true })
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
  return apiGet<CrateAccessDataResponse>(CRATE_ACCESS_DATA(crateName), undefined, { noCache: true })
}

/**
 * Set crate access data (download restrictions)
 */
export async function setCrateAccessData(
  crateName: string,
  downloadRestricted: boolean
): Promise<ApiResult<CrateAccessDataResponse>> {
  const data: CrateAccessDataRequest = { download_restricted: downloadRestricted }
  return apiPut<CrateAccessDataResponse>(CRATE_ACCESS_DATA(crateName), data)
}

// --- Documentation ---

/**
 * Request documentation build for a crate version
 */
export async function buildDocs(
  crateName: string,
  version: string
): Promise<ApiResult<void>> {
  return apiPost<void>(DOCS_BUILD, null, { package: crateName, version })
}

/**
 * Get documentation build queue
 */
export async function getDocsQueue(): Promise<ApiResult<DocQueueItem[]>> {
  // Backend returns { queue: [...] }, we need to extract the array
  const result = await apiGet<{ queue: DocQueueItem[] }>(DOCS_QUEUE, undefined, { noCache: true })
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
