/**
 * User API service
 */
import { apiGet, apiPost, apiPut, apiDelete } from './api'
import type { ApiResult } from '../types/api'
import type {
  User,
  UserCredentials,
  LoginCredentials,
  LoginResponse,
  PasswordResetResponse,
  ReadOnlyRequest,
  AdminRequest,
} from '../types/user'
import {
  ADD_USER,
  DELETE_USER,
  LIST_USERS,
  RESET_PWD,
  USER_READ_ONLY,
  USER_ADMIN,
  LOGIN,
  LOGOUT,
  LOGIN_STATE,
  CHANGE_PWD,
} from '../remote-routes'

const userErrorMessages: Record<number, string> = {
  400: 'Passwords do not match.',
  404: 'User not found.',
}

/**
 * Get list of all users
 */
export async function getUsers(): Promise<ApiResult<User[]>> {
  return apiGet<User[]>(LIST_USERS, undefined, { noCache: true })
}

/**
 * Add a new user
 */
export async function addUser(credentials: UserCredentials): Promise<ApiResult<void>> {
  return apiPost<void>(ADD_USER, credentials, undefined, {
    customErrors: userErrorMessages,
  })
}

/**
 * Delete a user
 */
export async function deleteUser(name: string): Promise<ApiResult<void>> {
  return apiDelete<void>(DELETE_USER(name), undefined, {
    customErrors: userErrorMessages,
  })
}

/**
 * Reset a user's password
 */
export async function resetPassword(name: string): Promise<ApiResult<PasswordResetResponse>> {
  return apiPut<PasswordResetResponse>(RESET_PWD(name), null)
}

/**
 * Set a user's read-only status
 */
export async function setReadOnly(
  name: string,
  state: boolean
): Promise<ApiResult<void>> {
  const data: ReadOnlyRequest = { state }
  return apiPost<void>(USER_READ_ONLY(name), data)
}

/**
 * Set a user's admin status
 */
export async function setAdmin(
  name: string,
  state: boolean
): Promise<ApiResult<void>> {
  const data: AdminRequest = { state }
  return apiPost<void>(USER_ADMIN(name), data, undefined, {
    customErrors: {
      400: 'Cannot remove your own admin rights.',
    },
  })
}

/**
 * Login with credentials
 */
export async function login(credentials: LoginCredentials): Promise<ApiResult<LoginResponse>> {
  return apiPost<LoginResponse>(LOGIN, credentials, undefined, {
    redirectOnAuthError: false,
    customErrors: {
      401: 'Wrong user or password',
      403: 'Account is locked or disabled.',
    },
  })
}

/**
 * Logout the current user
 */
export async function logout(): Promise<ApiResult<void>> {
  return apiPost<void>(LOGOUT, null, undefined, {
    redirectOnAuthError: false,
  })
}

/**
 * Get current login state
 */
export async function getLoginState(): Promise<ApiResult<LoginResponse>> {
  return apiGet<LoginResponse>(LOGIN_STATE, undefined, {
    noCache: true,
    redirectOnAuthError: false,
  })
}

/**
 * Change password
 */
export async function changePassword(
  oldPwd: string,
  newPwd1: string,
  newPwd2: string
): Promise<ApiResult<void>> {
  return apiPut<void>(
    CHANGE_PWD,
    { old_pwd: oldPwd, new_pwd1: newPwd1, new_pwd2: newPwd2 },
    {
      customErrors: {
        400: 'New passwords do not match.',
        401: 'Current password is incorrect.',
      },
    }
  )
}
