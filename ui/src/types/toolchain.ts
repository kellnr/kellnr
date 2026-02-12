/**
 * Toolchain types for rustup-compatible toolchain distribution
 */

/** Information about a specific target archive */
export interface ToolchainTarget {
  id: number
  target: string
  storage_path: string
  hash: string
  size: number
}

/** Toolchain with all its targets */
export interface Toolchain {
  id: number
  name: string
  version: string
  date: string
  channel: string | null
  created: string
  targets: ToolchainTarget[]
}

/** Channel information linking channel name to toolchain version */
export interface ChannelInfo {
  name: string
  version: string
  date: string
}

/** Request body for setting a channel */
export interface SetChannelRequest {
  name: string
  version: string
}

/** Response for toolchain operations */
export interface ToolchainResponse {
  success: boolean
  message?: string
}

/** Parameters for uploading a toolchain */
export interface ToolchainUploadParams {
  name: string
  version: string
  target: string
  date: string
  channel?: string
}
