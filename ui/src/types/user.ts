/**
 * User-related type definitions
 */

export interface User {
  name: string
  is_admin: boolean
  is_read_only: boolean
}

export interface UserCredentials {
  name: string
  pwd1: string
  pwd2: string
  is_admin: boolean
  is_read_only: boolean
}

export interface LoginCredentials {
  user: string
  pwd: string
  remember_me: boolean
}

export interface LoginResponse {
  user: string
  is_admin: boolean
}

export interface PasswordResetResponse {
  new_pwd: string
}

export interface ReadOnlyRequest {
  state: boolean
}

export interface AdminRequest {
  state: boolean
}
