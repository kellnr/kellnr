<template>
  <div>
    <SectionHeader icon="mdi-shield-key" title="Authentication Tokens" :count="tokens.length" />

    <div class="section-content">
      <p class="text-body-2 text-medium-emphasis mb-5">
        Authentication tokens allow you to publish crates via <code>cargo</code> without using your password.
        Tokens can be revoked at any time.
      </p>

      <!-- Existing Tokens -->
      <div v-if="tokens.length > 0" class="tokens-section mb-6">
        <SubsectionHeader icon="mdi-key-chain" title="Active Tokens" />

        <div class="list-container">
          <ListItem
            v-for="token in tokens"
            :key="token.id"
            icon="mdi-key"
            :title="token.name"
          >
            <template #actions>
              <v-btn
                size="small"
                color="error"
                variant="tonal"
                @click="handleDeleteToken(token)"
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
        icon="mdi-key-remove"
        message="No authentication tokens created yet."
        class="mb-6"
      />

      <!-- Add New Token Form -->
      <FormSection icon="mdi-key-plus" title="Create New Token">
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
      </FormSection>
    </div>

    <!-- Delete Confirmation Dialog -->
    <ConfirmDialog
      v-model="dialogOpen"
      :title="dialog.title"
      :message="dialog.message"
      sub-message="Any applications using this token will no longer be able to authenticate."
      confirm-text="Delete Token"
      confirm-color="error"
      confirm-icon="mdi-delete"
      @confirm="dialog.confirm()"
      @cancel="dialog.cancel()"
    />
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref, computed } from "vue"
import { useStatusMessage, useConfirmCallback } from "../composables"
import { tokenService } from "../services"
import { isSuccess } from "../services/api"
import type { Token } from "../types/token"
import {
  SectionHeader,
  SubsectionHeader,
  ListItem,
  EmptyState,
  FormSection,
  ConfirmDialog,
} from "./common"

// State
const tokens = ref<Token[]>([])
const tokenName = ref("")
const createdTokenValue = ref("")
const createLoading = ref(false)

// Composables
const createStatus = useStatusMessage()
const { dialog, showConfirm } = useConfirmCallback()

// Computed wrapper for dialog.isOpen
const dialogOpen = computed({
  get: () => dialog.isOpen.value,
  set: (val: boolean) => { dialog.isOpen.value = val }
})

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

.list-container {
  display: flex;
  flex-direction: column;
  gap: 8px;
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

/* Responsive */
@media (max-width: 600px) {
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
