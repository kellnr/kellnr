/**
 * Token API service
 */
import { apiGet, apiPost, apiDelete } from './api'
import type { ApiResult } from '../types/api'
import type { Token, TokenCreateRequest, TokenCreateResponse } from '../types/token'
import { ADD_TOKEN, DELETE_TOKEN, LIST_TOKENS } from '../remote-routes'

/**
 * Get list of all tokens for the current user
 */
export async function getTokens(): Promise<ApiResult<Token[]>> {
  return apiGet<Token[]>(LIST_TOKENS, undefined, { noCache: true })
}

/**
 * Create a new authentication token
 */
export async function createToken(name: string): Promise<ApiResult<TokenCreateResponse>> {
  const data: TokenCreateRequest = { name }
  return apiPost<TokenCreateResponse>(ADD_TOKEN, data, undefined, {
    customErrors: {
      400: 'Invalid token name.',
      409: 'A token with this name already exists.',
    },
  })
}

/**
 * Delete an authentication token
 */
export async function deleteToken(id: number): Promise<ApiResult<void>> {
  return apiDelete<void>(DELETE_TOKEN(id))
}
