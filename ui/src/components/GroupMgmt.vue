<template>
  <div>
    <!-- Header -->
    <div class="section-header">
      <v-icon icon="mdi-account-group" size="small" color="primary" class="me-3"></v-icon>
      <span class="text-h6 font-weight-bold">{{ editingGroup ? `Edit Group: ${editingGroup}` : 'Group Management' }}</span>
      <span v-if="!editingGroup && groups.length > 0" class="group-count">{{ groups.length }}</span>
      <v-btn v-if="editingGroup" variant="text" size="small" class="ms-auto" @click="cancelEditGroup">
        <v-icon icon="mdi-arrow-left" size="small" class="me-1"></v-icon>
        Back
      </v-btn>
    </div>

    <!-- Content -->
    <div class="section-content">
      <!-- Groups List View -->
      <template v-if="!editingGroup">
        <p class="text-body-2 text-medium-emphasis mb-5">
          Organize users into groups for easier access control. Groups can be assigned to crates to grant download permissions.
        </p>

        <!-- Groups List -->
        <div v-if="groups.length > 0" class="groups-section mb-6">
          <div class="subsection-header">
            <v-icon icon="mdi-folder-account" size="x-small" class="me-2" color="primary"></v-icon>
            <span class="text-body-2 font-weight-medium">Existing Groups</span>
          </div>

          <div class="group-list">
            <div v-for="group in groups" :key="group.name" class="group-item">
              <div class="group-info">
                <div class="group-avatar">
                  <v-icon icon="mdi-account-group" size="small"></v-icon>
                </div>
                <span class="group-name">{{ group.name }}</span>
              </div>
              <div class="group-actions">
                <v-btn
                  color="primary"
                  variant="tonal"
                  size="small"
                  @click="startEditGroup(group.name)"
                >
                  <v-icon icon="mdi-pencil-outline" size="small" class="me-1"></v-icon>
                  Edit
                </v-btn>

                <v-btn
                  color="error"
                  variant="tonal"
                  size="small"
                  @click="handleDeleteGroup(group.name)"
                >
                  <v-icon icon="mdi-delete-outline" size="small"></v-icon>
                </v-btn>
              </div>
            </div>
          </div>
        </div>

        <div v-else class="empty-state mb-6">
          <v-icon icon="mdi-account-group-outline" size="large" class="mb-3 text-medium-emphasis"></v-icon>
          <p class="text-body-2 text-medium-emphasis mb-0">No groups created yet.</p>
        </div>

        <!-- Add Group Form -->
        <div class="add-group-section">
          <div class="subsection-header">
            <v-icon icon="mdi-folder-plus" size="x-small" class="me-2" color="primary"></v-icon>
            <span class="text-body-2 font-weight-medium">Create New Group</span>
          </div>

          <v-form @submit.prevent="handleAddGroup" class="add-group-form">
            <div class="form-row">
              <v-text-field
                v-model="newGroupName"
                placeholder="Enter group name"
                prepend-inner-icon="mdi-account-group-outline"
                variant="outlined"
                density="comfortable"
                hide-details
                class="group-input"
              ></v-text-field>
              <v-btn color="primary" type="submit" size="large">
                <v-icon icon="mdi-plus" size="small" class="me-2"></v-icon>
                Create Group
              </v-btn>
            </div>
          </v-form>

          <v-alert
            v-if="addGroupStatus.hasStatus"
            :type="addGroupStatus.isSuccess ? 'success' : 'error'"
            variant="tonal"
            class="mt-4"
            closable
            @click:close="addGroupStatus.clear()"
          >
            {{ addGroupStatus.message }}
          </v-alert>
        </div>
      </template>

      <!-- Edit Group View -->
      <template v-else>
        <!-- Group Members Section -->
        <div class="members-section mb-6">
          <div class="subsection-header">
            <v-icon icon="mdi-account-multiple" size="x-small" class="me-2" color="primary"></v-icon>
            <span class="text-body-2 font-weight-medium">Group Members</span>
            <span class="member-count">{{ groupMembers.length }}</span>
          </div>

          <div v-if="groupMembers.length > 0" class="member-list">
            <div v-for="member in groupMembers" :key="member.name" class="member-item">
              <div class="member-info">
                <div class="member-avatar">
                  <v-icon icon="mdi-account" size="small"></v-icon>
                </div>
                <span class="member-name">{{ member.name }}</span>
              </div>
              <v-btn
                color="error"
                variant="text"
                size="small"
                @click="handleRemoveMember(member.name)"
              >
                <v-icon icon="mdi-close" size="small"></v-icon>
              </v-btn>
            </div>
          </div>

          <div v-else class="empty-state-small">
            <v-icon icon="mdi-account-off" size="small" class="me-2 text-medium-emphasis"></v-icon>
            <span class="text-body-2 text-medium-emphasis">No members in this group yet.</span>
          </div>

          <v-alert
            v-if="memberStatus.hasStatus"
            :type="memberStatus.isSuccess ? 'success' : 'error'"
            variant="tonal"
            class="mt-4"
            closable
            @click:close="memberStatus.clear()"
          >
            {{ memberStatus.message }}
          </v-alert>
        </div>

        <!-- Add Member Section -->
        <div class="add-member-section">
          <div class="subsection-header">
            <v-icon icon="mdi-account-plus" size="x-small" class="me-2" color="primary"></v-icon>
            <span class="text-body-2 font-weight-medium">Add Member</span>
          </div>

          <v-form @submit.prevent="handleAddMember" class="add-member-form">
            <div class="form-row">
              <v-select
                v-model="selectedUser"
                :items="availableUsers"
                item-title="name"
                item-value="name"
                placeholder="Select a user to add"
                prepend-inner-icon="mdi-account-search"
                variant="outlined"
                density="comfortable"
                hide-details
                :disabled="availableUsers.length === 0"
                class="member-select"
              ></v-select>
              <v-btn
                color="primary"
                type="submit"
                size="large"
                :disabled="!selectedUser || availableUsers.length === 0"
              >
                <v-icon icon="mdi-account-plus" size="small" class="me-2"></v-icon>
                Add
              </v-btn>
            </div>

            <p v-if="availableUsers.length === 0" class="text-body-2 text-medium-emphasis mt-3 mb-0">
              <v-icon icon="mdi-information-outline" size="small" class="me-1"></v-icon>
              All users are already members of this group.
            </p>
          </v-form>
        </div>
      </template>
    </div>

    <!-- Snackbar for notifications -->
    <v-snackbar
      v-model="notification.snackbar.show"
      :color="notification.snackbar.color"
      :timeout="notification.snackbar.timeout"
      location="bottom"
    >
      {{ notification.snackbar.message }}
      <template v-slot:actions>
        <v-btn variant="text" @click="notification.close()">Close</v-btn>
      </template>
    </v-snackbar>

    <!-- Confirmation Dialog -->
    <v-dialog v-model="dialogIsOpen" max-width="450">
      <v-card class="confirm-dialog">
        <div class="dialog-header">
          <v-icon icon="mdi-alert-circle" color="warning" size="small" class="me-3"></v-icon>
          <span class="text-h6 font-weight-bold">{{ dialog.title }}</span>
        </div>

        <v-card-text class="pa-5">
          <p class="text-body-1 mb-0">{{ dialog.message }}</p>
        </v-card-text>

        <v-card-actions class="pa-4 pt-0">
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="dialog.cancel()">Cancel</v-btn>
          <v-btn :color="dialog.confirmColor" variant="flat" @click="dialog.confirm()">Confirm</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref, computed } from "vue"
import { useStatusMessage, useConfirmCallback, useNotification } from "../composables"
import { groupService, userService } from "../services"
import { isSuccess } from "../services/api"
import type { Group, GroupUser } from "../types/group"
import type { User } from "../types/user"

// State
const groups = ref<Group[]>([])
const allUsers = ref<User[]>([])
const groupMembers = ref<GroupUser[]>([])
const newGroupName = ref("")
const editingGroup = ref("")
const selectedUser = ref("")

// Composables
const addGroupStatus = useStatusMessage()
const memberStatus = useStatusMessage()
const { dialog, showConfirm } = useConfirmCallback()
const dialogIsOpen = dialog.isOpen  // Destructure to make it a top-level ref for v-model
const notification = useNotification()

// Computed
const availableUsers = computed(() => {
  const memberNames = new Set(groupMembers.value.map(m => m.name))
  return allUsers.value.filter(user => !memberNames.has(user.name))
})

// Lifecycle
onBeforeMount(() => {
  loadGroups()
  loadAllUsers()
})

// Load groups from API
async function loadGroups() {
  const result = await groupService.getGroups()
  if (isSuccess(result)) {
    groups.value = result.data
  }
}

// Load all users from API
async function loadAllUsers() {
  const result = await userService.getUsers()
  if (isSuccess(result)) {
    allUsers.value = result.data
  }
}

// Load group members
async function loadGroupMembers() {
  if (!editingGroup.value) return

  const result = await groupService.getGroupUsers(editingGroup.value)
  if (isSuccess(result)) {
    groupMembers.value = result.data.users
  }
}

// Add a new group
async function handleAddGroup() {
  const name = newGroupName.value.trim()
  if (!name) return

  addGroupStatus.clear()

  const result = await groupService.createGroup(name)

  if (isSuccess(result)) {
    addGroupStatus.setSuccess("Group successfully created.")
    newGroupName.value = ""
    await loadGroups()
  } else {
    addGroupStatus.setError(result.error.message)
  }
}

// Delete a group with confirmation
function handleDeleteGroup(name: string) {
  showConfirm({
    title: "Delete Group",
    message: `Are you sure you want to delete group "${name}"? This action cannot be undone.`,
    confirmColor: "error",
    onConfirm: async () => {
      const result = await groupService.deleteGroup(name)
      if (isSuccess(result)) {
        notification.showSuccess(`Group "${name}" deleted`)
        await loadGroups()
      } else {
        notification.showError(result.error.message)
      }
    }
  })
}

// Start editing a group
function startEditGroup(groupName: string) {
  editingGroup.value = groupName
  selectedUser.value = ""
  memberStatus.clear()
  loadGroupMembers()
}

// Cancel editing
function cancelEditGroup() {
  editingGroup.value = ""
  selectedUser.value = ""
  groupMembers.value = []
  memberStatus.clear()
}

// Add a member to the group
async function handleAddMember() {
  if (!selectedUser.value || !editingGroup.value) return

  memberStatus.clear()

  const result = await groupService.addGroupUser(editingGroup.value, selectedUser.value)

  if (isSuccess(result)) {
    memberStatus.setSuccess("Member added to group.")
    selectedUser.value = ""
    await loadGroupMembers()
  } else {
    memberStatus.setError(result.error.message)
  }
}

// Remove a member from the group with confirmation
function handleRemoveMember(userName: string) {
  showConfirm({
    title: "Remove Member",
    message: `Are you sure you want to remove "${userName}" from this group?`,
    confirmColor: "error",
    onConfirm: async () => {
      const result = await groupService.removeGroupUser(editingGroup.value, userName)
      if (isSuccess(result)) {
        memberStatus.setSuccess("Member removed from group.")
        await loadGroupMembers()
      } else {
        memberStatus.setError(result.error.message)
      }
    }
  })
}
</script>

<style scoped>
.section-header {
  display: flex;
  align-items: center;
  padding: 16px 24px;
  background: rgba(var(--v-theme-primary), 0.05);
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.group-count {
  margin-left: auto;
  background: rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-primary));
  font-size: 12px;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 12px;
}

.section-content {
  padding: 24px;
}

/* Subsection Header */
.subsection-header {
  display: flex;
  align-items: center;
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.member-count {
  margin-left: auto;
  background: rgba(var(--v-theme-primary), 0.1);
  color: rgb(var(--v-theme-primary));
  font-size: 11px;
  font-weight: 500;
  padding: 2px 8px;
  border-radius: 10px;
}

/* Group List */
.group-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.group-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  transition: all 0.2s ease;
}

.group-item:hover {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.06);
}

.group-info {
  display: flex;
  align-items: center;
  gap: 12px;
}

.group-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  background: rgba(var(--v-theme-primary), 0.15);
  color: rgb(var(--v-theme-primary));
}

.group-name {
  font-weight: 600;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.group-actions {
  display: flex;
  gap: 8px;
}

/* Member List */
.member-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.member-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 6px;
  transition: all 0.2s ease;
}

.member-item:hover {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.03);
}

.member-info {
  display: flex;
  align-items: center;
  gap: 10px;
}

.member-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 6px;
  background: rgba(var(--v-theme-primary), 0.08);
  color: rgb(var(--v-theme-primary));
}

.member-name {
  font-weight: 500;
  font-size: 14px;
  color: rgb(var(--v-theme-on-surface));
}

/* Empty States */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 32px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px dashed rgb(var(--v-theme-outline));
  border-radius: 8px;
}

.empty-state-small {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  background: rgb(var(--v-theme-surface));
  border: 1px dashed rgb(var(--v-theme-outline));
  border-radius: 6px;
}

/* Add Group/Member Sections */
.add-group-section,
.add-member-section,
.members-section {
  padding: 20px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
}

.add-group-form,
.add-member-form {
  margin-top: 4px;
}

.form-row {
  display: flex;
  gap: 12px;
  align-items: center;
}

.group-input,
.member-select {
  flex: 1;
}

.group-input :deep(.v-field),
.member-select :deep(.v-field) {
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
}

/* Confirm Dialog */
.confirm-dialog {
  border-radius: 12px;
  overflow: hidden;
}

.dialog-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  background: rgba(var(--v-theme-warning), 0.08);
  border-bottom: 1px solid rgba(var(--v-theme-warning), 0.2);
}

/* Responsive */
@media (max-width: 600px) {
  .section-header {
    padding: 16px 20px;
  }

  .section-content {
    padding: 20px;
  }

  .form-row {
    flex-direction: column;
  }

  .group-input,
  .member-select {
    width: 100%;
  }

  .group-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }

  .group-actions {
    width: 100%;
  }
}
</style>
