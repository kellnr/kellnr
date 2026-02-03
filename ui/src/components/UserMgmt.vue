<template>
  <div>
    <SectionHeader icon="mdi-account-multiple" title="User Management" :count="users.length" />

    <div class="section-content">
      <p class="text-body-2 text-medium-emphasis mb-5">
        Manage user accounts, permissions, and access levels. Admins have full access, while read-only users can only view crates.
      </p>

      <!-- User List -->
      <div v-if="users.length > 0" class="users-section mb-6">
        <SubsectionHeader icon="mdi-account-group" title="Registered Users" />

        <div class="list-container">
          <ListItem
            v-for="user in users"
            :key="user.name"
            :icon="user.is_admin ? 'mdi-shield-account' : 'mdi-account'"
            :title="user.name"
            test-id="user-item"
          >
            <template #badges>
              <div class="user-badges">
                <span class="role-badge" :class="user.is_admin ? 'admin' : 'user'">
                  {{ user.is_admin ? 'Admin' : 'User' }}
                </span>
                <span v-if="user.is_read_only" class="role-badge readonly">
                  Read-only
                </span>
              </div>
            </template>
            <template #actions>
              <v-btn
                :color="user.is_admin ? 'warning' : 'primary'"
                variant="tonal"
                size="small"
                :disabled="user.name === currentUserName"
                @click="handleToggleAdmin(user)"
              >
                <v-icon :icon="user.is_admin ? 'mdi-shield-off-outline' : 'mdi-shield-crown-outline'" size="small" class="me-1"></v-icon>
                {{ user.is_admin ? 'Demote' : 'Promote' }}
              </v-btn>

              <v-btn
                :color="user.is_read_only ? 'info' : 'default'"
                variant="tonal"
                size="small"
                :disabled="user.name === currentUserName && !user.is_read_only"
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
                :disabled="user.name === currentUserName"
                @click="handleDeleteUser(user.name)"
              >
                <v-icon icon="mdi-delete-outline" size="small"></v-icon>
              </v-btn>
            </template>
          </ListItem>
        </div>
      </div>

      <EmptyState
        v-else
        icon="mdi-account-off"
        message="No users registered yet."
        class="mb-6"
      />

      <!-- Add User Form -->
      <FormSection icon="mdi-account-plus" title="Add New User">
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
      </FormSection>
    </div>

    <!-- Snackbar for notifications -->
    <NotificationSnackbar
      v-model="notification.snackbar.show"
      :message="notification.snackbar.message"
      :color="notification.snackbar.color"
      :timeout="notification.snackbar.timeout"
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
import { onBeforeMount, ref, computed } from 'vue'
import { useStatusMessage, useConfirmCallback, useNotification } from "../composables"
import { userService } from "../services"
import { isSuccess } from "../services/api"
import type { User } from "../types/user"
import { useStore } from "../store/store"
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
const users = ref<User[]>([])
const newUserName = ref("")
const newUserPwd1 = ref("")
const newUserPwd2 = ref("")
const newUserIsAdmin = ref(false)
const newUserIsReadOnly = ref(false)

// Store
const store = useStore()
const currentUserName = computed(() => store.loggedInUser)

// Composables
const addStatus = useStatusMessage()
const { dialog, showConfirm } = useConfirmCallback()
const notification = useNotification()

// Computed wrapper for dialog.isOpen
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
        user.is_read_only = newState
        notification.showSuccess(`"${user.name}" is now ${actionText}`)
      } else {
        notification.showError(result.error.message)
      }
    }
  })
}

// Toggle admin status with confirmation
function handleToggleAdmin(user: User) {
  const newState = !user.is_admin
  const actionText = newState ? "an admin" : "a regular user"

  showConfirm({
    title: "Change Admin Status",
    message: newState
      ? `Promote "${user.name}" to admin? They will have full access to all features.`
      : `Demote "${user.name}" from admin? They will lose administrative privileges.`,
    confirmColor: newState ? "primary" : "warning",
    onConfirm: async () => {
      const result = await userService.setAdmin(user.name, newState)
      if (isSuccess(result)) {
        user.is_admin = newState
        notification.showSuccess(`"${user.name}" is now ${actionText}`)
      } else {
        notification.showError(result.error.message)
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

/* Responsive */
@media (max-width: 768px) {
  .form-options {
    flex-direction: column;
    gap: 12px;
  }
}

@media (max-width: 600px) {
  .section-content {
    padding: 20px;
  }

  .form-grid {
    grid-template-columns: 1fr;
  }
}
</style>
