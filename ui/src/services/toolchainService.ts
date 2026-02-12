/**
 * Toolchain API service for rustup-compatible toolchain distribution
 */
import axios from 'axios'
import { apiGet, apiPut, apiDelete } from './api'
import type { ApiResult, ApiError } from '../types/api'
import type {
  Toolchain,
  ChannelInfo,
  SetChannelRequest,
  ToolchainResponse,
  ToolchainUploadParams,
} from '../types/toolchain'
import {
  TOOLCHAIN_LIST,
  TOOLCHAIN_DELETE,
  TOOLCHAIN_DELETE_TARGET,
  TOOLCHAIN_CHANNELS,
  TOOLCHAIN_SET_CHANNEL,
} from '../remote-routes'

/**
 * Get list of all toolchains
 */
export async function getToolchains(): Promise<ApiResult<Toolchain[]>> {
  return apiGet<Toolchain[]>(TOOLCHAIN_LIST, undefined, { noCache: true })
}

/**
 * Upload a toolchain archive
 *
 * Uses direct axios for binary upload with query params
 */
export async function uploadToolchain(
  params: ToolchainUploadParams,
  file: File
): Promise<ApiResult<ToolchainResponse>> {
  try {
    const queryParams = new URLSearchParams({
      name: params.name,
      version: params.version,
      target: params.target,
      date: params.date,
    })
    if (params.channel) {
      queryParams.set('channel', params.channel)
    }

    const response = await axios.put<ToolchainResponse>(
      `${TOOLCHAIN_LIST}?${queryParams.toString()}`,
      file,
      {
        withCredentials: true,
        headers: {
          'Content-Type': 'application/octet-stream',
        },
      }
    )
    return { data: response.data, error: null }
  } catch (err) {
    if (axios.isAxiosError(err) && err.response) {
      const status = err.response.status
      const responseData = err.response.data as ToolchainResponse | undefined
      const message =
        responseData?.message ||
        (status === 409
          ? 'Target already exists for this toolchain version.'
          : status === 403
            ? 'Admin access required.'
            : status === 503
              ? 'Toolchain storage not configured.'
              : 'Upload failed.')

      const apiError: ApiError = { status, message }
      return { data: null, error: apiError }
    }
    const apiError: ApiError = {
      status: 0,
      message: 'Network error. Please check your connection.',
    }
    return { data: null, error: apiError }
  }
}

/**
 * Delete an entire toolchain with all its targets
 */
export async function deleteToolchain(
  name: string,
  version: string
): Promise<ApiResult<ToolchainResponse>> {
  return apiDelete<ToolchainResponse>(TOOLCHAIN_DELETE(name, version), undefined, {
    customErrors: {
      403: 'Admin access required.',
      404: 'Toolchain not found.',
      503: 'Toolchain storage not configured.',
    },
  })
}

/**
 * Delete a toolchain target
 */
export async function deleteToolchainTarget(
  name: string,
  version: string,
  target: string
): Promise<ApiResult<ToolchainResponse>> {
  return apiDelete<ToolchainResponse>(TOOLCHAIN_DELETE_TARGET(name, version, target), undefined, {
    customErrors: {
      403: 'Admin access required.',
      404: 'Toolchain not found.',
      503: 'Toolchain storage not configured.',
    },
  })
}

/**
 * Get list of all channels
 */
export async function getChannels(): Promise<ApiResult<ChannelInfo[]>> {
  return apiGet<ChannelInfo[]>(TOOLCHAIN_CHANNELS, undefined, { noCache: true })
}

/**
 * Set a channel to point to a specific toolchain version
 */
export async function setChannel(
  channel: string,
  name: string,
  version: string
): Promise<ApiResult<ToolchainResponse>> {
  const data: SetChannelRequest = { name, version }
  return apiPut<ToolchainResponse>(TOOLCHAIN_SET_CHANNEL(channel), data, {
    customErrors: {
      403: 'Admin access required.',
      404: 'Toolchain not found.',
    },
  })
}
