/**
 * Group-related type definitions
 */

export interface Group {
  name: string
}

export interface GroupCreateRequest {
  name: string
}

export interface GroupUser {
  name: string
}

export interface GroupUsersResponse {
  users: GroupUser[]
}
