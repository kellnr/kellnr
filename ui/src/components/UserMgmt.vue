<template>
  <div>
    <SectionHeader icon="mdi-account-multiple" :title="SETTINGS_SECTION_TITLES.users" :count="users.length" role="heading" aria-level="2" />

    <div class="section-content">
      <p class="text-body-medium text-medium-emphasis mb-5">
        Manage user accounts, permissions, and access levels. Admins have full access, while read-only users can only view crates.
      </p>

      <!-- User List -->
      <div v-if="users.length > 0" class="users-section" :class="{ 'mb-6': !ssoEnforced }">
        <SubsectionHeader icon="mdi-account-group" title="Registered users" />

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
                <span class="role-badge" :class="roleOf(user)" data-testid="user-role-badge">
                  {{ roleLabel(roleOf(user)) }}
                </span>
                <span v-if="user.name === currentUserName" class="role-badge self" data-testid="user-self-badge">
                  You
                </span>
              </div>
            </template>
            <!--
              Each user has one of the three roles. Promote and Demote switch between admin and user,
              Lock and Unlock between read-only and user. Admins cannot demote, lock or delete themselves.
            -->
            <template #actions>
              <v-btn
                v-if="user.name !== currentUserName"
                variant="tonal"
                data-testid="user-toggle-admin"
                @click="handleChangeRole(user, roleOf(user) === 'admin' ? 'user' : 'admin')"
              >
                <v-icon :icon="roleOf(user) === 'admin' ? 'mdi-shield-off-outline' : 'mdi-shield-crown-outline'" size="small" class="me-1"></v-icon>
                {{ roleOf(user) === 'admin' ? 'Demote' : 'Promote' }}
              </v-btn>

              <v-btn
                v-if="roleOf(user) !== 'admin'"
                variant="tonal"
                data-testid="user-toggle-read-only"
                @click="handleChangeRole(user, roleOf(user) === 'read-only' ? 'user' : 'read-only')"
              >
                <v-icon :icon="roleOf(user) === 'read-only' ? 'mdi-lock-open-outline' : 'mdi-lock-outline'" size="small" class="me-1"></v-icon>
                {{ roleOf(user) === 'read-only' ? 'Unlock' : 'Lock' }}
              </v-btn>

              <v-btn
                v-if="!ssoEnforced"
                variant="tonal"
                data-testid="user-reset-password"
                @click="handleResetPassword(user.name)"
              >
                <v-icon icon="mdi-key-outline" size="small" class="me-1"></v-icon>
                Reset
              </v-btn>

              <v-btn
                v-if="user.name !== currentUserName"
                variant="tonal"
                class="kellnr-danger-btn"
                data-testid="user-delete"
                @click="handleDeleteUser(user.name)"
              >
                <v-icon icon="mdi-delete-outline" size="small" class="me-1"></v-icon>
                Delete
              </v-btn>
            </template>
          </ListItem>
        </div>
      </div>

      <EmptyState
        v-else
        icon="mdi-account-off"
        message="No users registered yet."
        :class="{ 'mb-6': !ssoEnforced }"
      />

      <!-- Add User Form -->
      <FormSection v-if="!ssoEnforced" icon="mdi-account-plus" title="Add new user">
        <v-form @submit.prevent="handleAddUser" class="add-user-form">
          <div class="form-grid">
            <div class="form-field">
              <label class="field-label" for="new-user-name">Username</label>
              <v-text-field
                id="new-user-name"
                v-model="newUserName"
                placeholder="Enter username"
                autocomplete="off"
                prepend-inner-icon="mdi-account-outline"
                variant="outlined"
                density="comfortable"
                hide-details
              ></v-text-field>
            </div>

            <div class="form-field">
              <label class="field-label" for="new-user-password">Password</label>
              <v-text-field
                id="new-user-password"
                v-model="newUserPwd1"
                placeholder="Enter password"
                prepend-inner-icon="mdi-lock-outline"
                type="password"
                autocomplete="new-password"
                variant="outlined"
                density="comfortable"
                hide-details
              ></v-text-field>
            </div>

            <div class="form-field">
              <label class="field-label" for="new-user-password-confirm">Confirm password</label>
              <v-text-field
                id="new-user-password-confirm"
                v-model="newUserPwd2"
                placeholder="Confirm password"
                prepend-inner-icon="mdi-lock-check-outline"
                type="password"
                autocomplete="new-password"
                variant="outlined"
                density="comfortable"
                hide-details
              ></v-text-field>
            </div>
          </div>

          <fieldset class="form-options">
            <legend class="field-label">Role</legend>
            <v-radio-group
              v-model="newUserRole"
              inline
              hide-details
              density="compact"
              data-testid="new-user-role"
            >
              <v-radio
                v-for="role in USER_ROLES"
                :key="role.value"
                :value="role.value"
                color="primary"
                class="role-option"
                :data-testid="`new-user-role-${role.value}`"
              >
                <template v-slot:label>
                  <div class="role-label">
                    <span class="role-title">{{ role.label }}</span>
                    <span class="role-desc">{{ role.description }}</span>
                  </div>
                </template>
              </v-radio>
            </v-radio-group>
          </fieldset>

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
              Create user
            </v-btn>
          </div>
        </v-form>
      </FormSection>
    </div>

    <!-- Snackbar for notifications -->
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
import { SETTINGS_SECTION_TITLES } from "../common/settingsSections"
import { USER_ROLES, roleChanges, roleFlags, roleLabel, roleOf, type UserRole } from "../common/userRole"
import "../common/actionButtons.css"
import type { User } from "../types/user"
import { useStore } from "../store/store"
import {
  SectionHeader,
  SubsectionHeader,
  ListItem,
  EmptyState,
  FormSection,
  ConfirmDialog,
} from "./common"

const { ssoEnforced } = defineProps<{ ssoEnforced: boolean }>()

// State
const users = ref<User[]>([])
const newUserName = ref("")
const newUserPwd1 = ref("")
const newUserPwd2 = ref("")
const newUserRole = ref<UserRole>("user")

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
    ...roleFlags(newUserRole.value),
  })

  if (isSuccess(result)) {
    addStatus.setSuccess("User successfully added.")
    // Clear form
    newUserName.value = ""
    newUserPwd1.value = ""
    newUserPwd2.value = ""
    newUserRole.value = "user"
    await loadUsers()
  } else {
    addStatus.setError(result.error.message)
  }
}

// Delete a user with confirmation
function handleDeleteUser(name: string) {
  showConfirm({
    title: "Delete user",
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
    title: "Reset password",
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

const ROLE_CHANGE_MESSAGES: Record<UserRole, (name: string) => string> = {
  admin: (name) => `Promote "${name}" to admin? They will have full access to all features.`,
  "read-only": (name) => `Make "${name}" read-only? They will only be able to view crates.`,
  user: (name) => `Make "${name}" a regular user? They will be able to view and publish crates, but not administer Kellnr.`,
}

async function applyRoleChanges(user: User, target: UserRole): Promise<string | null> {
  for (const change of roleChanges(user, target)) {
    const result = change.flag === "admin"
      ? await userService.setAdmin(user.name, change.state)
      : await userService.setReadOnly(user.name, change.state)
    if (!isSuccess(result)) {
      return result.error.message
    }
    if (change.flag === "admin") {
      user.is_admin = change.state
    } else {
      user.is_read_only = change.state
    }
  }
  return null
}

function handleChangeRole(user: User, target: UserRole) {
  showConfirm({
    title: "Change role",
    message: ROLE_CHANGE_MESSAGES[target](user.name),
    confirmColor: "primary",
    onConfirm: async () => {
      const error = await applyRoleChanges(user, target)
      if (error) {
        notification.showError(`Failed to change the role of "${user.name}": ${error}`)
        await loadUsers()
        return
      }
      notification.showSuccess(`"${user.name}" is now ${roleLabel(target).toLowerCase()}`)
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
  border-radius: var(--kellnr-radius);
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

.role-badge.read-only,
.role-badge.self {
  background: rgb(var(--v-theme-surface-variant));
  color: rgb(var(--v-theme-on-surface-variant));
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
  border-radius: var(--kellnr-radius);
  background: rgb(var(--v-theme-surface));
}

.form-options {
  margin: 0 0 16px;
  padding: 12px 16px 16px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--kellnr-radius);
}

.form-options legend {
  padding: 0 4px;
}

.role-option {
  margin-right: 24px;
}

.role-label {
  display: flex;
  flex-direction: column;
}

.role-title {
  font-size: 14px;
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface));
}

.role-desc {
  font-size: 12px;
  color: rgb(var(--v-theme-on-surface-variant));
}

.form-actions {
  padding-top: 16px;
  border-top: 1px solid rgb(var(--v-theme-outline));
}

/* Responsive */
@media (max-width: 600px) {
  .section-content {
    padding: 20px;
  }

  .form-grid {
    grid-template-columns: 1fr;
  }
}
</style>
