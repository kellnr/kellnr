/**
 * Settings API service
 */
import { apiGet, isSuccess } from './api'
import type { ApiResult } from '../types/api'
import type { DocsEnabled, Settings, Version } from '../types/settings'
import type { OAuth2Config } from '../types/oauth2'
import { SETTINGS, DOCS_ENABLED, VERSION, OAUTH2_CONFIG } from '../remote-routes'

/**
 * Get startup configuration settings with source tracking
 */
export async function getSettings(): Promise<ApiResult<Settings>> {
  return apiGet<Settings>(SETTINGS, undefined, {
    redirectOnAuthError: false,
  })
}

/**
 * Check if documentation generation is enabled
 */
export async function getDocsEnabled(): Promise<ApiResult<DocsEnabled>> {
  return apiGet<DocsEnabled>(DOCS_ENABLED, undefined, {
    redirectOnAuthError: false,
  })
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
let oauth2ConfigCache: OAuth2Config | null = null

export async function getOAuth2Config(): Promise<ApiResult<OAuth2Config>> {
  if (oauth2ConfigCache) {
    return { data: oauth2ConfigCache, error: null }
  }
  const result = await apiGet<OAuth2Config>(OAUTH2_CONFIG, undefined, {
    redirectOnAuthError: false,
  })
  if (isSuccess(result)) {
    oauth2ConfigCache = result.data
  }
  return result
}
