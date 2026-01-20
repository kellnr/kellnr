<template>
  <div>
    <!-- Header -->
    <div class="section-header">
      <v-icon icon="mdi-shield-key" size="small" color="primary" class="me-3"></v-icon>
      <span class="text-h6 font-weight-bold">Authentication Tokens</span>
      <span v-if="tokens.length > 0" class="token-count">{{ tokens.length }}</span>
    </div>

    <!-- Content -->
    <div class="section-content">
      <p class="text-body-2 text-medium-emphasis mb-5">
        Authentication tokens allow you to publish crates via <code>cargo</code> without using your password.
        Tokens can be revoked at any time.
      </p>

      <!-- Existing Tokens -->
      <div v-if="tokens.length > 0" class="tokens-section mb-6">
        <div class="subsection-header">
          <v-icon icon="mdi-key-chain" size="x-small" class="me-2" color="primary"></v-icon>
          <span class="text-body-2 font-weight-medium">Active Tokens</span>
        </div>

        <div class="token-list">
          <div v-for="token in tokens" :key="token.id" class="token-item">
            <div class="token-info">
              <v-icon icon="mdi-key" size="small" class="me-3 token-icon"></v-icon>
              <span class="token-name">{{ token.name }}</span>
            </div>
            <v-btn
              size="small"
              color="error"
              variant="tonal"
              @click="handleDeleteToken(token)"
            >
              <v-icon icon="mdi-delete-outline" size="small" class="me-1"></v-icon>
              Delete
            </v-btn>
          </div>
        </div>
      </div>

      <div v-else class="empty-state mb-6">
        <v-icon icon="mdi-key-remove" size="large" class="mb-3 text-medium-emphasis"></v-icon>
        <p class="text-body-2 text-medium-emphasis mb-0">No authentication tokens created yet.</p>
      </div>

      <!-- Add New Token Form -->
      <div class="add-token-section">
        <div class="subsection-header">
          <v-icon icon="mdi-key-plus" size="x-small" class="me-2" color="primary"></v-icon>
          <span class="text-body-2 font-weight-medium">Create New Token</span>
        </div>

        <v-form @submit.prevent="handleCreateToken" class="token-form">
          <div class="form-row">
            <v-text-field
              v-model="tokenName"
              placeholder="Enter a descriptive name for the token"
              prepend-inner-icon="mdi-tag-outline"
              variant="outlined"
              density="comfortable"
              hide-details
              class="token-input"
            ></v-text-field>
            <v-btn
              color="primary"
              type="submit"
              :loading="createLoading"
              size="large"
            >
              <v-icon icon="mdi-plus" size="small" class="me-2"></v-icon>
              Create Token
            </v-btn>
          </div>
        </v-form>

        <!-- Token Created Alert -->
        <v-alert
          v-if="createStatus.isSuccess"
          type="success"
          variant="tonal"
          closable
          @click:close="createStatus.clear()"
          class="mt-4"
        >
          <div class="token-created">
            <div class="d-flex align-center mb-2">
              <v-icon icon="mdi-check-circle" size="small" class="me-2"></v-icon>
              <span class="font-weight-medium">{{ createStatus.message }}</span>
            </div>

            <div v-if="createdTokenValue" class="token-display">
              <code class="token-value">{{ createdTokenValue }}</code>
              <v-btn
                color="primary"
                variant="flat"
                @click="copyToken"
                size="small"
              >
                <v-icon icon="mdi-content-copy" size="small" class="me-1"></v-icon>
                Copy
              </v-btn>
            </div>
          </div>
        </v-alert>

        <!-- Error Alert -->
        <v-alert
          v-if="createStatus.isError"
          type="error"
          variant="tonal"
          closable
          @click:close="createStatus.clear()"
          class="mt-4"
        >
          {{ createStatus.message }}
        </v-alert>
      </div>
    </div>

    <!-- Delete Confirmation Dialog -->
    <v-dialog v-model="dialog.isOpen" max-width="450">
      <v-card class="delete-dialog">
        <div class="dialog-header">
          <v-icon icon="mdi-alert-circle" color="error" size="small" class="me-3"></v-icon>
          <span class="text-h6 font-weight-bold">{{ dialog.title }}</span>
        </div>

        <v-card-text class="pa-5">
          <p class="text-body-1 mb-2">
            {{ dialog.message }}
          </p>
          <p class="text-body-2 text-medium-emphasis mb-0">
            Any applications using this token will no longer be able to authenticate.
          </p>
        </v-card-text>

        <v-card-actions class="pa-4 pt-0">
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="dialog.cancel()">
            Cancel
          </v-btn>
          <v-btn color="error" variant="flat" @click="dialog.confirm()">
            <v-icon icon="mdi-delete" size="small" class="me-1"></v-icon>
            Delete Token
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref } from "vue"
import { useStatusMessage, useConfirmCallback } from "../composables"
import { tokenService } from "../services"
import { isSuccess } from "../services/api"
import type { Token } from "../types/token"

// State
const tokens = ref<Token[]>([])
const tokenName = ref("")
const createdTokenValue = ref("")
const createLoading = ref(false)

// Composables
const createStatus = useStatusMessage()
const { dialog, showConfirm } = useConfirmCallback()

// Lifecycle
onBeforeMount(() => {
  loadTokens()
})

// Load tokens from API
async function loadTokens() {
  const result = await tokenService.getTokens()
  if (isSuccess(result)) {
    tokens.value = result.data
  }
}

// Create a new token
async function handleCreateToken() {
  const name = tokenName.value.trim()
  if (!name) {
    createStatus.setError("Please enter a name for the token")
    return
  }

  createLoading.value = true
  createStatus.clear()

  const result = await tokenService.createToken(name)

  createLoading.value = false

  if (isSuccess(result)) {
    createdTokenValue.value = result.data.token
    createStatus.setSuccess("Token created! Copy and save it now — it won't be shown again.")
    tokenName.value = ""
    await loadTokens()
  } else {
    createStatus.setError(result.error.message)
  }
}

// Handle delete token with confirmation
function handleDeleteToken(token: Token) {
  showConfirm({
    title: "Delete Token",
    message: `Are you sure you want to delete the token "${token.name}"?`,
    confirmColor: "error",
    onConfirm: async () => {
      const result = await tokenService.deleteToken(token.id)
      if (isSuccess(result)) {
        await loadTokens()
      }
    }
  })
}

// Copy token to clipboard
function copyToken() {
  navigator.clipboard.writeText(createdTokenValue.value)
    .then(() => {
      const originalMessage = createStatus.message.value
      createStatus.setSuccess("Token copied to clipboard!")
      setTimeout(() => {
        createStatus.setSuccess(originalMessage)
      }, 2000)
    })
    .catch(err => {
      console.error("Failed to copy token:", err)
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

.token-count {
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

.section-content code {
  background: rgba(var(--v-theme-primary), 0.08);
  color: rgb(var(--v-theme-primary));
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 13px;
  font-family: 'Roboto Mono', monospace;
}

/* Subsection Header */
.subsection-header {
  display: flex;
  align-items: center;
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

/* Token List */
.token-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.token-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  transition: all 0.2s ease;
}

.token-item:hover {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.06);
}

.token-info {
  display: flex;
  align-items: center;
}

.token-icon {
  color: rgb(var(--v-theme-primary));
  opacity: 0.7;
}

.token-name {
  font-weight: 500;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
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

/* Add Token Section */
.add-token-section {
  padding: 20px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
}

.token-form {
  margin-top: 4px;
}

.form-row {
  display: flex;
  gap: 12px;
  align-items: center;
}

.token-input {
  flex: 1;
}

.token-input :deep(.v-field) {
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
}

/* Token Created Display */
.token-created {
  width: 100%;
}

.token-display {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  margin-top: 8px;
}

.token-value {
  flex: 1;
  word-break: break-all;
  font-family: 'Roboto Mono', monospace;
  font-size: 13px;
  background: transparent !important;
  padding: 0 !important;
  color: rgb(var(--v-theme-on-surface));
}

/* Delete Dialog */
.delete-dialog {
  border-radius: 12px;
  overflow: hidden;
}

.dialog-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  background: rgba(var(--v-theme-error), 0.08);
  border-bottom: 1px solid rgba(var(--v-theme-error), 0.2);
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

  .token-input {
    width: 100%;
  }

  .token-display {
    flex-direction: column;
    align-items: stretch;
  }
}
</style>
