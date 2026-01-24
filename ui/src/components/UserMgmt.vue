<template>
  <div>
    <!-- Header -->
    <div class="section-header">
      <v-icon icon="mdi-account-multiple" size="small" color="primary" class="me-3"></v-icon>
      <span class="text-h6 font-weight-bold">User Management</span>
      <span v-if="users.length > 0" class="user-count">{{ users.length }}</span>
    </div>

    <!-- Content -->
    <div class="section-content">
      <p class="text-body-2 text-medium-emphasis mb-5">
        Manage user accounts, permissions, and access levels. Admins have full access, while read-only users can only view crates.
      </p>

      <!-- User List -->
      <div v-if="users.length > 0" class="users-section mb-6">
        <div class="subsection-header">
          <v-icon icon="mdi-account-group" size="x-small" class="me-2" color="primary"></v-icon>
          <span class="text-body-2 font-weight-medium">Registered Users</span>
        </div>

        <div class="user-list">
          <div v-for="user in users" :key="user.name" class="user-item">
            <div class="user-info">
              <div class="user-avatar" :class="{ admin: user.is_admin }">
                <v-icon :icon="user.is_admin ? 'mdi-shield-account' : 'mdi-account'" size="small"></v-icon>
              </div>
              <div class="user-details">
                <span class="user-name">{{ user.name }}</span>
                <div class="user-badges">
                  <span class="role-badge" :class="user.is_admin ? 'admin' : 'user'">
                    {{ user.is_admin ? 'Admin' : 'User' }}
                  </span>
                  <span v-if="user.is_read_only" class="role-badge readonly">
                    Read-only
                  </span>
                </div>
              </div>
            </div>
            <div class="user-actions">
              <v-btn
                :color="user.is_read_only ? 'info' : 'default'"
                variant="tonal"
                size="small"
                @click="handleToggleReadOnly(user)"
              >
                <v-icon :icon="user.is_read_only ? 'mdi-lock-open-outline' : 'mdi-lock-outline'" size="small" class="me-1"></v-icon>
                {{ user.is_read_only ? 'Unlock' : 'Lock' }}
              </v-btn>

              <v-btn
                color="warning"
                variant="tonal"
                size="small"
                @click="handleResetPassword(user.name)"
              >
                <v-icon icon="mdi-key-outline" size="small" class="me-1"></v-icon>
                Reset
              </v-btn>

              <v-btn
                color="error"
                variant="tonal"
                size="small"
                @click="handleDeleteUser(user.name)"
              >
                <v-icon icon="mdi-delete-outline" size="small"></v-icon>
              </v-btn>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="empty-state mb-6">
        <v-icon icon="mdi-account-off" size="large" class="mb-3 text-medium-emphasis"></v-icon>
        <p class="text-body-2 text-medium-emphasis mb-0">No users registered yet.</p>
      </div>

      <!-- Add User Form -->
      <div class="add-user-section">
        <div class="subsection-header">
          <v-icon icon="mdi-account-plus" size="x-small" class="me-2" color="primary"></v-icon>
          <span class="text-body-2 font-weight-medium">Add New User</span>
        </div>

        <v-form @submit.prevent="handleAddUser" class="add-user-form">
          <div class="form-grid">
            <div class="form-field">
              <label class="field-label">Username</label>
              <v-text-field
                v-model="newUserName"
                placeholder="Enter username"
                prepend-inner-icon="mdi-account-outline"
                variant="outlined"
                density="comfortable"
                hide-details
              ></v-text-field>
            </div>

            <div class="form-field">
              <label class="field-label">Password</label>
              <v-text-field
                v-model="newUserPwd1"
                placeholder="Enter password"
                prepend-inner-icon="mdi-lock-outline"
                type="password"
                variant="outlined"
                density="comfortable"
                hide-details
              ></v-text-field>
            </div>

            <div class="form-field">
              <label class="field-label">Confirm Password</label>
              <v-text-field
                v-model="newUserPwd2"
                placeholder="Confirm password"
                prepend-inner-icon="mdi-lock-check-outline"
                type="password"
                variant="outlined"
                density="comfortable"
                hide-details
              ></v-text-field>
            </div>
          </div>

          <div class="form-options">
            <v-checkbox
              v-model="newUserIsAdmin"
              label="Administrator"
              hide-details
              density="compact"
              class="option-checkbox"
            >
              <template v-slot:label>
                <div class="checkbox-label">
                  <span class="checkbox-title">Administrator</span>
                  <span class="checkbox-desc">Full access to all features</span>
                </div>
              </template>
            </v-checkbox>

            <v-checkbox
              v-model="newUserIsReadOnly"
              label="Read-only"
              hide-details
              density="compact"
              class="option-checkbox"
            >
              <template v-slot:label>
                <div class="checkbox-label">
                  <span class="checkbox-title">Read-only</span>
                  <span class="checkbox-desc">Can only view crates</span>
                </div>
              </template>
            </v-checkbox>
          </div>

          <v-alert
            v-if="addStatus.hasStatus"
            :type="addStatus.isSuccess ? 'success' : 'error'"
            variant="tonal"
            class="mt-4"
            closable
            @click:close="addStatus.clear()"
          >
            {{ addStatus.message }}
          </v-alert>

          <div class="form-actions">
            <v-btn type="submit" color="primary" size="large">
              <v-icon icon="mdi-account-plus" size="small" class="me-2"></v-icon>
              Create User
            </v-btn>
          </div>
        </v-form>
      </div>
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
    <v-dialog v-model="dialogOpen" max-width="450">
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
import { onBeforeMount, ref, computed } from 'vue'
import { useStatusMessage, useConfirmCallback, useNotification } from "../composables"
import { userService } from "../services"
import { isSuccess } from "../services/api"
import type { User } from "../types/user"

// State
const users = ref<User[]>([])
const newUserName = ref("")
const newUserPwd1 = ref("")
const newUserPwd2 = ref("")
const newUserIsAdmin = ref(false)
const newUserIsReadOnly = ref(false)

// Composables
const addStatus = useStatusMessage()
const { dialog, showConfirm } = useConfirmCallback()
const notification = useNotification()

// Computed wrapper for dialog.isOpen to properly handle ref unwrapping in v-model
const dialogOpen = computed({
  get: () => dialog.isOpen.value,
  set: (val: boolean) => { dialog.isOpen.value = val }
})

// Lifecycle
onBeforeMount(() => {
  loadUsers()
})

// Load users from API
async function loadUsers() {
  const result = await userService.getUsers()
  if (isSuccess(result)) {
    users.value = result.data
  }
}

// Add a new user
async function handleAddUser() {
  addStatus.clear()

  const result = await userService.addUser({
    name: newUserName.value,
    pwd1: newUserPwd1.value,
    pwd2: newUserPwd2.value,
    is_admin: newUserIsAdmin.value,
    is_read_only: newUserIsReadOnly.value,
  })

  if (isSuccess(result)) {
    addStatus.setSuccess("User successfully added.")
    // Clear form
    newUserName.value = ""
    newUserPwd1.value = ""
    newUserPwd2.value = ""
    newUserIsAdmin.value = false
    newUserIsReadOnly.value = false
    await loadUsers()
  } else {
    addStatus.setError(result.error.message)
  }
}

// Delete a user with confirmation
function handleDeleteUser(name: string) {
  showConfirm({
    title: "Delete User",
    message: `Are you sure you want to delete user "${name}"? This action cannot be undone.`,
    confirmColor: "error",
    onConfirm: async () => {
      const result = await userService.deleteUser(name)
      if (isSuccess(result)) {
        notification.showSuccess(`User "${name}" deleted`)
        await loadUsers()
      } else {
        notification.showError(result.error.message)
      }
    }
  })
}

// Reset a user's password with confirmation
function handleResetPassword(name: string) {
  showConfirm({
    title: "Reset Password",
    message: `Are you sure you want to reset the password for "${name}"?`,
    confirmColor: "warning",
    onConfirm: async () => {
      const result = await userService.resetPassword(name)
      if (isSuccess(result)) {
        notification.showSuccess(
          `Password for "${name}" reset to "${result.data.new_pwd}". Notify the user to change the password on the next login.`
        )
      } else {
        notification.showError(result.error.message)
      }
    }
  })
}

// Toggle read-only status with confirmation
function handleToggleReadOnly(user: User) {
  const newState = !user.is_read_only
  const actionText = newState ? "read-only" : "unlocked"

  showConfirm({
    title: "Change Read-only Status",
    message: newState
      ? `Make "${user.name}" read-only? They will only be able to view crates.`
      : `Remove read-only restriction from "${user.name}"?`,
    confirmColor: "primary",
    onConfirm: async () => {
      const result = await userService.setReadOnly(user.name, newState)
      if (isSuccess(result)) {
        // Update local state
        user.is_read_only = newState
        notification.showSuccess(`"${user.name}" is now ${actionText}`)
      } else {
        notification.showError(result.error.message)
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

.user-count {
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

/* User List */
.user-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.user-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  transition: all 0.2s ease;
}

.user-item:hover {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.06);
}

.user-info {
  display: flex;
  align-items: center;
  gap: 14px;
}

.user-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: 10px;
  background: rgba(var(--v-theme-primary), 0.08);
  color: rgb(var(--v-theme-primary));
}

.user-avatar.admin {
  background: rgba(var(--v-theme-primary), 0.15);
  color: rgb(var(--v-theme-primary));
}

.user-details {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.user-name {
  font-weight: 600;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.user-badges {
  display: flex;
  gap: 6px;
}

.role-badge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}

.role-badge.admin {
  background: rgba(var(--v-theme-primary), 0.15);
  color: rgb(var(--v-theme-primary));
}

.role-badge.user {
  background: rgba(var(--v-theme-info), 0.15);
  color: rgb(var(--v-theme-info));
}

.role-badge.readonly {
  background: rgba(var(--v-theme-warning), 0.15);
  color: rgb(var(--v-theme-warning));
}

.user-actions {
  display: flex;
  gap: 8px;
}

/* Empty State */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 32px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px dashed rgb(var(--v-theme-outline));
  border-radius: 8px;
}

/* Add User Section */
.add-user-section {
  padding: 20px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
}

.add-user-form {
  margin-top: 4px;
}

.form-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 16px;
  margin-bottom: 20px;
}

.form-field {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.field-label {
  font-size: 13px;
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface));
}

.form-field :deep(.v-field) {
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
}

.form-options {
  display: flex;
  gap: 24px;
  padding: 16px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  margin-bottom: 16px;
}

.option-checkbox {
  flex: 1;
}

.checkbox-label {
  display: flex;
  flex-direction: column;
}

.checkbox-title {
  font-size: 14px;
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface));
}

.checkbox-desc {
  font-size: 12px;
  color: rgb(var(--v-theme-on-surface-variant));
}

.form-actions {
  padding-top: 16px;
  border-top: 1px solid rgb(var(--v-theme-outline));
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
@media (max-width: 768px) {
  .user-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }

  .user-actions {
    width: 100%;
    justify-content: flex-start;
  }

  .form-options {
    flex-direction: column;
    gap: 12px;
  }
}

@media (max-width: 600px) {
  .section-header {
    padding: 16px 20px;
  }

  .section-content {
    padding: 20px;
  }

  .form-grid {
    grid-template-columns: 1fr;
  }
}
</style>
