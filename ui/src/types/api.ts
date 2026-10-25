/**
 * Generic API response types
 */

export interface ApiError {
  status: number
  message: string
}

export interface ApiResult<T> {
  data: T | null
  error: ApiError | null
}

/**
 * Paginated response wrapper
 */
export interface PaginatedResponse<T> {
  items: T[]
  page: number
  pageSize: number
  total: number
}

/**
 * Crate list response from the API
 */
export interface CratesResponse {
  crates: import('./crate_overview').CrateOverview[]
  page: number
  /** Number of crates on all pages */
  total: number
}

/**
 * Order of the crate list
 */
export type CrateSort = 'name' | 'updated' | 'downloads'

/** Restricts the crate list to crates with a keyword and/or category */
export type CrateTagFilter = { keyword?: string, category?: string }

/**
 * Search response from the API
 */
export interface SearchResponse {
  crates: import('./crate_overview').CrateOverview[]
  page: number
  /** Number of matching crates on all pages */
  total: number
}

/**
 * Crate users response
 */
export interface CrateUsersResponse {
  users: { login: string }[]
}

/**
 * Crate groups response
 */
export interface CrateGroupsResponse {
  groups: { name: string }[]
}

/**
 * Crate owners response
 */
export interface CrateOwnersResponse {
  users: { login: string }[]
}

/**
 * Crate access data
 */
export interface CrateAccessDataResponse {
  download_restricted: boolean
}

export interface CrateAccessDataRequest {
  download_restricted: boolean
}
