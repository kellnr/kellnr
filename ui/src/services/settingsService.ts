/**
 * Settings API service
 */
import { apiGet } from './api'
import type { ApiResult } from '../types/api'
import type { Settings, Version } from '../types/settings'
import { SETTINGS, VERSION } from '../remote-routes'

/**
 * Get startup configuration settings
 */
export async function getSettings(): Promise<ApiResult<Settings>> {
  return apiGet<Settings>(SETTINGS)
}

/**
 * Get Kellnr version
 */
export async function getVersion(): Promise<ApiResult<Version>> {
  return apiGet<Version>(VERSION, undefined, {
    redirectOnAuthError: false,
  })
}
