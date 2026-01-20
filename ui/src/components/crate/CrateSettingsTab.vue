<template>
  <div class="crate-settings-tab">
    <!-- Crate Owners -->
    <v-card class="mb-4 content-card settings-card" elevation="0">
      <div class="settings-header">
        <v-icon icon="mdi-shield-account" size="small" class="settings-icon"></v-icon>
        <span class="settings-title">Crate Owners</span>
        <span class="settings-count">{{ crateOwners.length }}</span>
      </div>
      <v-card-text class="settings-content">
        <v-alert v-if="!canManageOwners" type="info" variant="tonal" class="mb-4">
          Only existing crate owners or admins can add/remove crate owners.
        </v-alert>

        <div v-if="crateOwners.length > 0" class="settings-list">
          <div v-for="owner in crateOwners" :key="owner.login" class="settings-list-item">
            <div class="settings-list-item-info">
              <v-icon icon="mdi-account" size="small" class="me-3"></v-icon>
              <span class="settings-list-item-name">{{ owner.login }}</span>
            </div>
            <v-btn :disabled="!canManageOwners" color="error" variant="tonal" size="small"
              @click="handleDeleteOwner(owner.login)">
              <v-icon icon="mdi-delete-outline" size="small" class="me-1"></v-icon>
              Remove
            </v-btn>
          </div>
        </div>
        <p v-else class="text-body-2 text-disabled mb-0">No owners assigned yet.</p>

        <v-alert v-if="deleteOwnerStatus.hasStatus" :type="deleteOwnerStatus.isSuccess ? 'success' : 'error'"
          closable @click:close="deleteOwnerStatus.clear()" class="mt-4">
          {{ deleteOwnerStatus.message }}
        </v-alert>

        <div class="settings-form-section">
          <div class="settings-form-header">
            <v-icon icon="mdi-account-plus" size="small" class="me-2"></v-icon>
            <span>Add crate owner</span>
          </div>
          <v-form @submit.prevent="handleAddOwner" class="settings-form">
            <v-text-field v-model="newOwnerName" placeholder="Enter username" prepend-inner-icon="mdi-account-star"
              variant="outlined" density="comfortable" :disabled="!canManageOwners" hide-details class="settings-input"></v-text-field>
            <v-btn :disabled="!canManageOwners" color="primary" type="submit" variant="flat">
              <v-icon icon="mdi-plus" size="small" class="me-1"></v-icon>
              Add
            </v-btn>
          </v-form>
          <v-alert v-if="addOwnerStatus.hasStatus" :type="addOwnerStatus.isSuccess ? 'success' : 'error'"
            closable @click:close="addOwnerStatus.clear()" class="mt-3">
            {{ addOwnerStatus.message }}
          </v-alert>
        </div>
      </v-card-text>
    </v-card>

    <!-- Access Control -->
    <v-card class="mb-4 content-card settings-card" elevation="0">
      <div class="settings-header">
        <v-icon icon="mdi-lock-outline" size="small" class="settings-icon"></v-icon>
        <span class="settings-title">Access Control</span>
      </div>
      <v-card-text class="settings-content">
        <!-- Crate Access Rules -->
        <div class="settings-subsection">
          <div class="settings-subsection-header">
            <v-icon icon="mdi-shield-lock" size="x-small" class="me-2"></v-icon>
            <span>Download Restrictions</span>
          </div>
          <v-form @submit.prevent="handleSaveAccessData">
            <v-checkbox v-model="isDownloadRestricted" hide-details
              label="Restrict downloads to crate users only" class="settings-checkbox"></v-checkbox>
            <p class="settings-help-text">
              When enabled, only users explicitly added as crate users can download this crate.
              Requires <code>auth_required = true</code> in kellnr configuration.
            </p>
            <v-alert v-if="accessStatus.hasStatus"
              :type="accessStatus.isSuccess ? 'success' : 'error'" closable
              @click:close="accessStatus.clear()" class="mb-3">
              {{ accessStatus.message }}
            </v-alert>
            <v-btn color="primary" type="submit" variant="flat" size="small">
              <v-icon icon="mdi-content-save" size="small" class="me-1"></v-icon>
              Save Changes
            </v-btn>
          </v-form>
        </div>

        <!-- Crate Users -->
        <div class="settings-subsection">
          <div class="settings-subsection-header">
            <v-icon icon="mdi-account-multiple" size="x-small" class="me-2"></v-icon>
            <span>Crate Users</span>
            <span class="settings-count-small">{{ crateUsers.length }}</span>
          </div>

          <div v-if="crateUsers.length > 0" class="settings-list compact">
            <div v-for="user in crateUsers" :key="user.login" class="settings-list-item">
              <div class="settings-list-item-info">
                <v-icon icon="mdi-account" size="small" class="me-3"></v-icon>
                <span class="settings-list-item-name">{{ user.login }}</span>
              </div>
              <v-btn color="error" variant="text" size="small" @click="handleDeleteUser(user.login)">
                <v-icon icon="mdi-close" size="small"></v-icon>
              </v-btn>
            </div>
          </div>
          <p v-else class="text-body-2 text-disabled mb-4">No users assigned yet.</p>

          <v-alert v-if="deleteUserStatus.hasStatus"
            :type="deleteUserStatus.isSuccess ? 'success' : 'error'" closable
            @click:close="deleteUserStatus.clear()" class="mb-3">
            {{ deleteUserStatus.message }}
          </v-alert>

          <v-form @submit.prevent="handleAddUser" class="settings-form inline">
            <v-text-field v-model="newUserName" placeholder="Enter username" prepend-inner-icon="mdi-account"
              variant="outlined" density="compact" hide-details class="settings-input"></v-text-field>
            <v-btn color="primary" type="submit" variant="tonal" size="small">
              <v-icon icon="mdi-plus" size="small"></v-icon>
            </v-btn>
          </v-form>
          <v-alert v-if="addUserStatus.hasStatus" :type="addUserStatus.isSuccess ? 'success' : 'error'"
            closable @click:close="addUserStatus.clear()" class="mt-3">
            {{ addUserStatus.message }}
          </v-alert>
        </div>

        <!-- Crate Groups -->
        <div class="settings-subsection last">
          <div class="settings-subsection-header">
            <v-icon icon="mdi-account-group" size="x-small" class="me-2"></v-icon>
            <span>Crate Groups</span>
            <span class="settings-count-small">{{ crateGroups.length }}</span>
          </div>

          <div v-if="crateGroups.length > 0" class="settings-list compact">
            <div v-for="group in crateGroups" :key="group.name" class="settings-list-item">
              <div class="settings-list-item-info">
                <v-icon icon="mdi-account-group" size="small" class="me-3"></v-icon>
                <span class="settings-list-item-name">{{ group.name }}</span>
              </div>
              <v-btn color="error" variant="text" size="small" @click="handleDeleteGroup(group.name)">
                <v-icon icon="mdi-close" size="small"></v-icon>
              </v-btn>
            </div>
          </div>
          <p v-else class="text-body-2 text-disabled mb-4">No groups assigned yet.</p>

          <v-alert v-if="deleteGroupStatus.hasStatus"
            :type="deleteGroupStatus.isSuccess ? 'success' : 'error'" closable
            @click:close="deleteGroupStatus.clear()" class="mb-3">
            {{ deleteGroupStatus.message }}
          </v-alert>

          <v-form @submit.prevent="handleAddGroup" class="settings-form inline">
            <v-select v-model="newGroupName" :items="availableGroups" label="Select group"
              prepend-inner-icon="mdi-account-group" variant="outlined" density="compact" hide-details class="settings-input" />
            <v-btn color="primary" type="submit" variant="tonal" size="small">
              <v-icon icon="mdi-plus" size="small"></v-icon>
            </v-btn>
          </v-form>
          <v-alert v-if="addGroupStatus.hasStatus" :type="addGroupStatus.isSuccess ? 'success' : 'error'"
            closable @click:close="addGroupStatus.clear()" class="mt-3">
            {{ addGroupStatus.message }}
          </v-alert>
        </div>
      </v-card-text>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useStore } from '../../store/store'
import { useStatusMessage } from '../../composables'
import { crateService } from '../../services'
import { groupService } from '../../services'
import { isSuccess } from '../../services/api'
import type { CrateGroup } from '../../types/crate_data'

// Props
const props = defineProps<{
  crateName: string
}>()

// Emits
const emit = defineEmits<{
  (e: 'owners-changed', owners: string[]): void
}>()

// Router and store
const router = useRouter()
const store = useStore()

// Status messages
const addOwnerStatus = useStatusMessage()
const deleteOwnerStatus = useStatusMessage()
const addUserStatus = useStatusMessage()
const deleteUserStatus = useStatusMessage()
const addGroupStatus = useStatusMessage()
const deleteGroupStatus = useStatusMessage()
const accessStatus = useStatusMessage()

// State
const crateOwners = ref<{ login: string }[]>([])
const newOwnerName = ref('')
const crateUsers = ref<{ login: string }[]>([])
const newUserName = ref('')
const crateGroups = ref<CrateGroup[]>([])
const allGroups = ref<CrateGroup[]>([])
const newGroupName = ref('')
const isDownloadRestricted = ref(false)

// Computed
const canManageOwners = computed(() => {
  return store.loggedInUserIsAdmin || crateOwners.value.some(o => o.login === store.loggedInUser)
})

const availableGroups = computed(() => {
  const assigned = new Set(crateGroups.value.map(g => g.name))
  return allGroups.value.filter(group => !assigned.has(group.name)).map(group => group.name)
})

// Load data on mount
onMounted(() => {
  loadAllData()
})

async function loadAllData() {
  await Promise.all([
    loadOwners(),
    loadUsers(),
    loadGroups(),
    loadAllGroups(),
    loadAccessData()
  ])
}

// Owner management
async function loadOwners() {
  const result = await crateService.getCrateOwners(props.crateName)
  if (isSuccess(result)) {
    crateOwners.value = result.data.users ?? []
    emit('owners-changed', crateOwners.value.map(o => o.login))
  } else {
    crateOwners.value = []
    emit('owners-changed', [])
  }
}

async function handleAddOwner() {
  if (!canManageOwners.value) {
    addOwnerStatus.setError('Not allowed. Only existing owners or admins can add owners.')
    return
  }

  const result = await crateService.addCrateOwner(props.crateName, newOwnerName.value)
  if (isSuccess(result)) {
    addOwnerStatus.setSuccess('Crate owner successfully added.')
    newOwnerName.value = ''
    await loadOwners()
  } else {
    if (result.error.status === 401) {
      router.push('/login')
    } else if (result.error.status === 403) {
      addOwnerStatus.setError('Not allowed. Only existing owners or admins can add owners.')
    } else if (result.error.status === 404) {
      addOwnerStatus.setError('User not found. Did you provide an existing user name?')
    } else {
      addOwnerStatus.setError('Crate owner could not be added.')
    }
  }
}

async function handleDeleteOwner(name: string) {
  if (!canManageOwners.value) {
    deleteOwnerStatus.setError('Not allowed. Only existing owners or admins can remove owners.')
    return
  }

  if ((crateOwners.value?.length ?? 0) <= 1) {
    deleteOwnerStatus.setError('A crate must have at least one owner.')
    return
  }

  if (!confirm(`Delete crate owner "${name}"?`)) return

  const result = await crateService.deleteCrateOwner(props.crateName, name)
  if (isSuccess(result)) {
    deleteOwnerStatus.setSuccess('Crate owner successfully deleted.')
    await loadOwners()
  } else {
    if (result.error.status === 401) {
      router.push('/login')
    } else if (result.error.status === 403) {
      deleteOwnerStatus.setError('Not allowed. Only existing owners or admins can remove owners.')
    } else if (result.error.status === 409) {
      deleteOwnerStatus.setError('A crate must have at least one owner.')
    } else {
      deleteOwnerStatus.setError('Crate owner could not be deleted.')
    }
  }
}

// User management
async function loadUsers() {
  const result = await crateService.getCrateUsers(props.crateName)
  if (isSuccess(result)) {
    crateUsers.value = result.data.users ?? []
  } else {
    crateUsers.value = []
  }
}

async function handleAddUser() {
  const result = await crateService.addCrateUser(props.crateName, newUserName.value)
  if (isSuccess(result)) {
    addUserStatus.setSuccess('Crate user successfully added.')
    newUserName.value = ''
    await loadUsers()
  } else {
    if (result.error.status === 401 || result.error.status === 403) {
      router.push('/login')
    } else if (result.error.status === 404) {
      addUserStatus.setError('User not found. Did you provide an existing user name?')
    } else {
      addUserStatus.setError('Crate user could not be added.')
    }
  }
}

async function handleDeleteUser(name: string) {
  if (!confirm(`Delete crate user "${name}"?`)) return

  const result = await crateService.deleteCrateUser(props.crateName, name)
  if (isSuccess(result)) {
    deleteUserStatus.setSuccess('Crate user successfully deleted.')
    await loadUsers()
  } else {
    if (result.error.status === 404) {
      router.push('/login')
    } else {
      deleteUserStatus.setError('Crate user could not be deleted.')
    }
  }
}

// Group management
async function loadGroups() {
  const result = await crateService.getCrateGroups(props.crateName)
  if (isSuccess(result)) {
    crateGroups.value = result.data.groups ?? []
  } else {
    crateGroups.value = []
  }
}

async function loadAllGroups() {
  const result = await groupService.getGroups()
  if (isSuccess(result)) {
    allGroups.value = result.data ?? []
  } else {
    allGroups.value = []
  }
}

async function handleAddGroup() {
  const result = await crateService.addCrateGroup(props.crateName, newGroupName.value)
  if (isSuccess(result)) {
    addGroupStatus.setSuccess('Crate group successfully added.')
    newGroupName.value = ''
    await loadGroups()
  } else {
    if (result.error.status === 401 || result.error.status === 403) {
      router.push('/login')
    } else if (result.error.status === 404) {
      addGroupStatus.setError('Group not found. Did you provide an existing group name?')
    } else {
      addGroupStatus.setError('Crate group could not be added.')
    }
  }
}

async function handleDeleteGroup(name: string) {
  if (!confirm(`Delete crate group "${name}"?`)) return

  const result = await crateService.deleteCrateGroup(props.crateName, name)
  if (isSuccess(result)) {
    deleteGroupStatus.setSuccess('Crate group successfully deleted.')
    await loadGroups()
  } else {
    if (result.error.status === 404) {
      router.push('/login')
    } else {
      deleteGroupStatus.setError('Crate group could not be deleted.')
    }
  }
}

// Access data management
async function loadAccessData() {
  const result = await crateService.getCrateAccessData(props.crateName)
  if (isSuccess(result)) {
    isDownloadRestricted.value = result.data.download_restricted
  }
}

async function handleSaveAccessData() {
  const result = await crateService.setCrateAccessData(props.crateName, isDownloadRestricted.value)
  if (isSuccess(result)) {
    if (result.data.download_restricted) {
      accessStatus.setSuccess('Crate access restricted to crate users only.')
    } else {
      accessStatus.setSuccess('Crate access open to all.')
    }
    await loadAccessData()
  } else {
    if (result.error.status === 401 || result.error.status === 403) {
      router.push('/login')
    } else {
      accessStatus.setError('Crate access data could not be changed.')
    }
  }
}
</script>

<style scoped>
/* Content Cards */
.content-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
}

/* Settings Card Styles */
.settings-card {
  overflow: hidden;
}

.settings-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  background: rgb(var(--v-theme-surface-variant));
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.settings-icon {
  color: rgb(var(--v-theme-primary));
  margin-right: 12px;
}

.settings-title {
  font-weight: 600;
  font-size: 16px;
  color: rgb(var(--v-theme-on-surface));
}

.settings-count {
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface-variant));
  font-size: 12px;
  font-weight: 500;
  padding: 2px 10px;
  border-radius: 12px;
  margin-left: auto;
}

.settings-content {
  padding: 20px !important;
}

.settings-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 16px;
}

.settings-list.compact {
  margin-bottom: 12px;
}

.settings-list-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  transition: all 0.2s ease;
}

.settings-list-item:hover {
  border-color: rgb(var(--v-theme-primary));
}

.settings-list-item-info {
  display: flex;
  align-items: center;
}

.settings-list-item-info .v-icon {
  color: rgb(var(--v-theme-on-surface-variant));
}

.settings-list-item-name {
  font-weight: 500;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.settings-form-section {
  margin-top: 24px;
  padding-top: 20px;
  border-top: 1px solid rgb(var(--v-theme-outline));
}

.settings-form-header {
  display: flex;
  align-items: center;
  font-weight: 500;
  font-size: 14px;
  color: rgb(var(--v-theme-on-surface-variant));
  margin-bottom: 12px;
}

.settings-form-header .v-icon {
  color: rgb(var(--v-theme-primary));
}

.settings-form {
  display: flex;
  gap: 12px;
  align-items: flex-start;
}

.settings-form.inline {
  align-items: center;
}

.settings-input {
  flex: 1;
}

.settings-input :deep(.v-field) {
  margin-bottom: 0;
}

.settings-subsection {
  padding: 20px;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  margin-bottom: 16px;
}

.settings-subsection.last {
  margin-bottom: 0;
}

.settings-subsection-header {
  display: flex;
  align-items: center;
  font-weight: 600;
  font-size: 14px;
  color: rgb(var(--v-theme-on-surface));
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.settings-subsection-header .v-icon {
  color: rgb(var(--v-theme-primary));
}

.settings-count-small {
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface-variant));
  font-size: 11px;
  font-weight: 500;
  padding: 1px 8px;
  border-radius: 10px;
  margin-left: auto;
}

.settings-checkbox {
  margin-bottom: 8px;
}

.settings-checkbox :deep(.v-label) {
  font-size: 14px;
}

.settings-help-text {
  font-size: 13px;
  line-height: 1.5;
  color: rgb(var(--v-theme-on-surface-variant));
  margin-bottom: 16px;
  padding-left: 32px;
}

.settings-help-text code {
  background: rgb(var(--v-theme-surface));
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 12px;
  font-family: 'Roboto Mono', monospace;
}

/* Responsive */
@media (max-width: 600px) {
  .settings-form {
    flex-direction: column;
  }

  .settings-form.inline {
    flex-direction: row;
  }

  .settings-input {
    width: 100%;
  }
}
</style>
