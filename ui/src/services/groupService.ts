/**
 * Group API service
 */
import { apiGet, apiPost, apiPut, apiDelete } from './api'
import type { ApiResult } from '../types/api'
import type { Group, GroupCreateRequest, GroupUsersResponse } from '../types/group'
import {
  ADD_GROUP,
  DELETE_GROUP,
  LIST_GROUPS,
  GROUP_USERS,
  GROUP_USER,
} from '../remote-routes'

const groupErrorMessages: Record<number, string> = {
  400: 'Invalid group name.',
  404: 'Group not found.',
  409: 'A group with this name already exists.',
}

/**
 * Get list of all groups
 */
export async function getGroups(): Promise<ApiResult<Group[]>> {
  return apiGet<Group[]>(LIST_GROUPS, undefined, { noCache: true })
}

/**
 * Create a new group
 */
export async function createGroup(name: string): Promise<ApiResult<void>> {
  const data: GroupCreateRequest = { name }
  return apiPost<void>(ADD_GROUP, data, undefined, {
    customErrors: groupErrorMessages,
  })
}

/**
 * Delete a group
 */
export async function deleteGroup(name: string): Promise<ApiResult<void>> {
  return apiDelete<void>(DELETE_GROUP(name), undefined, {
    customErrors: groupErrorMessages,
  })
}

/**
 * Get users in a group
 */
export async function getGroupUsers(groupName: string): Promise<ApiResult<GroupUsersResponse>> {
  return apiGet<GroupUsersResponse>(GROUP_USERS(groupName), undefined, { noCache: true })
}

/**
 * Add a user to a group
 */
export async function addGroupUser(groupName: string, userName: string): Promise<ApiResult<void>> {
  return apiPut<void>(GROUP_USER(groupName, userName), null, {
    customErrors: {
      404: 'User not found.',
      409: 'User is already a member of this group.',
    },
  })
}

/**
 * Remove a user from a group
 */
export async function removeGroupUser(
  groupName: string,
  userName: string
): Promise<ApiResult<void>> {
  return apiDelete<void>(GROUP_USER(groupName, userName), undefined, {
    customErrors: {
      404: 'User not found in group.',
    },
  })
}
