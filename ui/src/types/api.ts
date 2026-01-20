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
}

/**
 * Search response from the API
 */
export interface SearchResponse {
  crates: import('./crate_overview').CrateOverview[]
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
