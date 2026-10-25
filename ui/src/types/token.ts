/**
 * Authentication token type definitions
 */

export interface Token {
  id: number
  name: string
  /** UTC date, missing for tokens from before Kellnr stored it */
  created?: string | null
  /** UTC date of the last use, updated at most once per hour */
  last_used?: string | null
}

export interface TokenCreateRequest {
  name: string
}

export interface TokenCreateResponse {
  token: string
}
