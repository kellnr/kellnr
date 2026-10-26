/**
 * Settings API service
 */
import { apiGet, isSuccess } from './api'
import type { ApiResult } from '../types/api'
import type { DocsEnabled, RegistryConfig, Settings, SponsorLinksEnabled, Version } from '../types/settings'
import type { OAuth2Config } from '../types/oauth2'
import {
  SETTINGS,
  DOCS_ENABLED,
  SPONSOR_LINKS_ENABLED,
  VERSION,
  OAUTH2_CONFIG,
  REGISTRY_CONFIG,
} from '../remote-routes'

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
 * Check if the UI shows links to sponsor Kellnr. The setting only changes
 * with a restart, so the first successful answer is cached.
 */
let sponsorLinksEnabledCache: SponsorLinksEnabled | null = null

export async function getSponsorLinksEnabled(): Promise<ApiResult<SponsorLinksEnabled>> {
  if (sponsorLinksEnabledCache) {
    return { data: sponsorLinksEnabledCache, error: null }
  }
  const result = await apiGet<SponsorLinksEnabled>(SPONSOR_LINKS_ENABLED, undefined, {
    redirectOnAuthError: false,
  })
  if (isSuccess(result)) {
    sponsorLinksEnabledCache = result.data
  }
  return result
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
 * Get the registry config that cargo reads
 */
export async function getRegistryConfig(): Promise<ApiResult<RegistryConfig>> {
  return apiGet<RegistryConfig>(REGISTRY_CONFIG)
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
