/**
 * Services barrel export
 *
 * Usage:
 * ```ts
 * import { userService, groupService, crateService, tokenService } from '@/services'
 * // or
 * import * as userService from '@/services/userService'
 * ```
 */

// Re-export all services as namespaces
export * as userService from './userService'
export * as groupService from './groupService'
export * as crateService from './crateService'
export * as tokenService from './tokenService'
export * as settingsService from './settingsService'

// Re-export API utilities
export { apiGet, apiPost, apiPut, apiDelete, isSuccess, isError } from './api'
export type { ApiRequestOptions } from './api'
