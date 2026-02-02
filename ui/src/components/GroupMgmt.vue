<template>
  <div>
    <SectionHeader
      :icon="editingGroup ? 'mdi-account-group' : 'mdi-account-group'"
      :title="editingGroup ? `Edit Group: ${editingGroup}` : 'Group Management'"
      :count="!editingGroup ? groups.length : undefined"
    >
      <template v-if="editingGroup" #actions>
        <v-btn variant="text" size="small" class="ms-auto" @click="cancelEditGroup">
          <v-icon icon="mdi-arrow-left" size="small" class="me-1"></v-icon>
          Back
        </v-btn>
      </template>
    </SectionHeader>

    <div class="section-content">
      <!-- Groups List View -->
      <template v-if="!editingGroup">
        <p class="text-body-2 text-medium-emphasis mb-5">
          Organize users into groups for easier access control. Groups can be assigned to crates to grant download permissions.
        </p>

        <!-- Groups List -->
        <div v-if="groups.length > 0" class="groups-section mb-6">
          <SubsectionHeader icon="mdi-folder-account" title="Existing Groups" />

          <div class="list-container">
            <ListItem
              v-for="group in groups"
              :key="group.name"
              icon="mdi-account-group"
              :title="group.name"
            >
              <template #actions>
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
              </template>
            </ListItem>
          </div>
        </div>

        <EmptyState
          v-else
          icon="mdi-account-group-outline"
          message="No groups created yet."
          class="mb-6"
        />

        <!-- Add Group Form -->
        <FormSection icon="mdi-folder-plus" title="Create New Group">
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
        </FormSection>
      </template>

      <!-- Edit Group View -->
      <template v-else>
        <!-- Group Members Section -->
        <div class="members-section mb-6">
          <SubsectionHeader icon="mdi-account-multiple" title="Group Members" :count="groupMembers.length" />

          <div v-if="groupMembers.length > 0" class="list-container">
            <ListItem
              v-for="member in groupMembers"
              :key="member.name"
              icon="mdi-account"
              :title="member.name"
              compact
            >
              <template #actions>
                <v-btn
                  color="error"
                  variant="text"
                  size="small"
                  @click="handleRemoveMember(member.name)"
                >
                  <v-icon icon="mdi-close" size="small"></v-icon>
                </v-btn>
              </template>
            </ListItem>
          </div>

          <EmptyState
            v-else
            icon="mdi-account-off"
            message="No members in this group yet."
            compact
          />

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
        <FormSection icon="mdi-account-plus" title="Add Member">
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
        </FormSection>
      </template>
    </div>

    <!-- Snackbar for notifications -->
    <NotificationSnackbar
      :snackbar="notification.snackbar"
      @close="notification.close()"
    />

    <!-- Confirmation Dialog -->
    <ConfirmDialog
      v-model="dialogOpen"
      :title="dialog.title"
      :message="dialog.message"
      :confirm-color="dialog.confirmColor"
      @confirm="dialog.confirm()"
      @cancel="dialog.cancel()"
    />
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref, computed } from "vue"
import { useStatusMessage, useConfirmCallback, useNotification } from "../composables"
import { groupService, userService } from "../services"
import { isSuccess } from "../services/api"
import type { Group, GroupUser } from "../types/group"
import type { User } from "../types/user"
import {
  SectionHeader,
  SubsectionHeader,
  ListItem,
  EmptyState,
  FormSection,
  ConfirmDialog,
  NotificationSnackbar,
} from "./common"

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
const notification = useNotification()

// Computed wrapper for dialog.isOpen
const dialogOpen = computed({
  get: () => dialog.isOpen.value,
  set: (val: boolean) => { dialog.isOpen.value = val }
})

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
.section-content {
  padding: 24px;
}

.list-container {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

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

/* Responsive */
@media (max-width: 600px) {
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
}
</style>
