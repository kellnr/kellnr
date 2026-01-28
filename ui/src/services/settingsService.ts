/**
 * Settings API service
 */
import { apiGet } from './api'
import type { ApiResult } from '../types/api'
import type { Settings, Version } from '../types/settings'
import type { OAuth2Config } from '../types/oauth2'
import { SETTINGS, VERSION, OAUTH2_CONFIG } from '../remote-routes'

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

/**
 * Get OAuth2/OIDC configuration
 * Returns whether OAuth2 is enabled and the button text to display
 */
export async function getOAuth2Config(): Promise<ApiResult<OAuth2Config>> {
  return apiGet<OAuth2Config>(OAUTH2_CONFIG, undefined, {
    redirectOnAuthError: false,
  })
}
