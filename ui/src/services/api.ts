/**
 * Base API service with centralized error handling
 */
import axios, { type AxiosRequestConfig, type AxiosError } from 'axios'
import type { ApiResult, ApiError } from '../types/api'
import router from '../router'

/**
 * Standard error messages for common HTTP status codes
 */
const ERROR_MESSAGES: Record<number, string> = {
  400: 'Invalid request. Please check your input.',
  401: 'Unauthorized. Please log in.',
  403: 'Access denied. You do not have permission.',
  404: 'Resource not found.',
  409: 'Conflict. The resource already exists or cannot be modified.',
  500: 'Server error. Please try again later.',
}

/**
 * Get a user-friendly error message from an axios error
 */
function getErrorMessage(error: AxiosError, customMessages?: Record<number, string>): string {
  if (!error.response) {
    return 'Network error. Please check your connection.'
  }

  const status = error.response.status

  // Check custom messages first
  if (customMessages?.[status]) {
    return customMessages[status]
  }

  // Fall back to standard messages
  return ERROR_MESSAGES[status] || 'An unexpected error occurred.'
}

/**
 * Check if the error requires redirect to login
 */
function shouldRedirectToLogin(status: number): boolean {
  return status === 401 || status === 403
}

/**
 * Base API configuration
 */
const apiConfig: AxiosRequestConfig = {
  withCredentials: true,
}

/**
 * Options for API requests
 */
export interface ApiRequestOptions {
  /** Custom error messages keyed by HTTP status code */
  customErrors?: Record<number, string>
  /** Whether to redirect to login on 401/403 (default: true) */
  redirectOnAuthError?: boolean
  /** Whether to disable caching */
  noCache?: boolean
}

/**
 * Perform a GET request
 */
export async function apiGet<T>(
  url: string,
  params?: Record<string, unknown>,
  options: ApiRequestOptions = {}
): Promise<ApiResult<T>> {
  try {
    const config: AxiosRequestConfig = {
      ...apiConfig,
      params,
    }

    if (options.noCache) {
      // @ts-expect-error axios-cache-interceptor adds cache option
      config.cache = false
    }

    const response = await axios.get<T>(url, config)
    return { data: response.data, error: null }
  } catch (err) {
    return handleError(err as AxiosError, options)
  }
}

/**
 * Perform a POST request
 */
export async function apiPost<T>(
  url: string,
  data?: unknown,
  params?: Record<string, unknown>,
  options: ApiRequestOptions = {}
): Promise<ApiResult<T>> {
  try {
    const config: AxiosRequestConfig = {
      ...apiConfig,
      params,
    }

    const response = await axios.post<T>(url, data, config)
    return { data: response.data, error: null }
  } catch (err) {
    return handleError(err as AxiosError, options)
  }
}

/**
 * Perform a PUT request
 */
export async function apiPut<T>(
  url: string,
  data?: unknown,
  options: ApiRequestOptions = {}
): Promise<ApiResult<T>> {
  try {
    const response = await axios.put<T>(url, data, apiConfig)
    return { data: response.data, error: null }
  } catch (err) {
    return handleError(err as AxiosError, options)
  }
}

/**
 * Perform a DELETE request
 */
export async function apiDelete<T>(
  url: string,
  params?: Record<string, unknown>,
  options: ApiRequestOptions = {}
): Promise<ApiResult<T>> {
  try {
    const config: AxiosRequestConfig = {
      ...apiConfig,
      params,
    }

    const response = await axios.delete<T>(url, config)
    return { data: response.data, error: null }
  } catch (err) {
    return handleError(err as AxiosError, options)
  }
}

/**
 * Handle API errors consistently
 */
function handleError<T>(
  error: AxiosError,
  options: ApiRequestOptions
): ApiResult<T> {
  const status = error.response?.status || 0
  const message = getErrorMessage(error, options.customErrors)

  // Redirect to login if needed
  if (options.redirectOnAuthError !== false && shouldRedirectToLogin(status)) {
    router.push('/login')
  }

  const apiError: ApiError = { status, message }
  return { data: null, error: apiError }
}

/**
 * Check if a result is successful
 */
export function isSuccess<T>(result: ApiResult<T>): result is { data: T; error: null } {
  return result.error === null && result.data !== null
}

/**
 * Check if a result is an error
 */
export function isError<T>(result: ApiResult<T>): result is { data: null; error: ApiError } {
  return result.error !== null
}
