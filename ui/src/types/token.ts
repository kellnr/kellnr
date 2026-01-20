/**
 * Authentication token type definitions
 */

export interface Token {
  id: number
  name: string
}

export interface TokenCreateRequest {
  name: string
}

export interface TokenCreateResponse {
  token: string
}
