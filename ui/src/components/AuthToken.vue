<template>
  <div>
    <SectionHeader icon="mdi-shield-key" :title="SETTINGS_SECTION_TITLES.tokens" :count="tokens.length" role="heading" aria-level="2" />

    <div class="section-content">
      <p class="text-body-medium text-medium-emphasis mb-5">
        Authentication tokens allow you to publish crates via <code>cargo</code> without using your password.
        Tokens can be revoked at any time.
      </p>

      <!-- The token is shown only once, so it stays until the user dismisses it -->
      <v-alert
        v-if="createdToken"
        type="success"
        variant="tonal"
        closable
        close-label="Dismiss token"
        class="mb-6"
        data-testid="token-created"
        @click:close="createdToken = null"
      >
        <div class="token-created">
          <p class="font-weight-medium mb-1">Token "{{ createdToken.name }}" created</p>
          <p class="text-body-medium mb-3">Copy it now, it will not be shown again.</p>

          <p class="text-body-medium mb-0">1. Run this command:</p>
          <CodeBlock :code="loginCommand" class="token-code" />

          <p class="text-body-medium mb-0">2. Paste the token when cargo asks for it:</p>
          <CodeBlock :code="createdToken.value" language="plaintext" class="token-code" data-testid="token-value" />
        </div>
      </v-alert>

      <!-- Existing Tokens -->
      <div v-if="tokens.length > 0" class="tokens-section mb-6">
        <SubsectionHeader icon="mdi-key-chain" title="Active tokens" />

        <div class="list-container">
          <ListItem
            v-for="token in tokens"
            :key="token.id"
            icon="mdi-key"
            :title="token.name"
            test-id="token-item"
          >
            <template #subtitle>
              <span class="text-body-small token-dates" data-testid="token-dates">
                Created <span :title="token.created ? exactDate(token.created) : undefined">{{
                  token.created ? fromNow(token.created) : "at an unknown time" }}</span>
                &middot;
                <template v-if="token.last_used">
                  Last used <span :title="exactDate(token.last_used)">{{ fromNow(token.last_used) }}</span>
                </template>
                <template v-else>Never used</template>
              </span>
            </template>
            <template #actions>
              <v-btn
                variant="tonal"
                class="kellnr-danger-btn"
                data-testid="token-revoke"
                @click="handleRevokeToken(token)"
              >
                <v-icon icon="mdi-cancel" size="small" class="me-1"></v-icon>
                Revoke
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
      <FormSection icon="mdi-key-plus" title="Create new token">
        <v-form @submit.prevent="handleCreateToken" class="token-form">
          <div class="form-row">
            <v-text-field
              v-model="tokenName"
              aria-label="Token name"
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
              Create token
            </v-btn>
          </div>
        </v-form>

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

    <NotificationSnackbar
      v-model="notification.snackbar.show"
      :message="notification.snackbar.message"
      :color="notification.snackbar.color"
      :timeout="notification.snackbar.timeout"
    />

    <ConfirmDialog
      v-model="dialogOpen"
      :title="dialog.title"
      :message="dialog.message"
      sub-message="Any applications using this token will no longer be able to authenticate."
      :confirm-text="dialog.confirmText"
      confirm-color="error"
      confirm-icon="mdi-cancel"
      @confirm="dialog.confirm()"
      @cancel="dialog.cancel()"
    />
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref, computed } from "vue"
import { useStatusMessage, useConfirmCallback, useRelativeTime, useNotification } from "../composables"
import { tokenService } from "../services"
import { isSuccess } from "../services/api"
import { SETTINGS_SECTION_TITLES } from "../common/settingsSections"
import { cargoLoginCommand } from "../common/tokenSetup"
import "../common/actionButtons.css"
import type { Token } from "../types/token"
import {
  SectionHeader,
  SubsectionHeader,
  ListItem,
  EmptyState,
  FormSection,
  ConfirmDialog,
  CodeBlock,
  NotificationSnackbar,
} from "./common"

interface CreatedToken {
  name: string
  value: string
}

const loginCommand = cargoLoginCommand()

// State
const tokens = ref<Token[]>([])
const tokenName = ref("")
const createdToken = ref<CreatedToken | null>(null)
const createLoading = ref(false)

// Composables
const createStatus = useStatusMessage()
const notification = useNotification()
const { fromNow, exactDate } = useRelativeTime()
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

async function loadTokens() {
  const result = await tokenService.getTokens()
  if (isSuccess(result)) {
    tokens.value = result.data
  } else {
    notification.showError(`Failed to load tokens: ${result.error.message}`)
  }
}

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
    createdToken.value = { name, value: result.data.token }
    tokenName.value = ""
    await loadTokens()
  } else {
    createStatus.setError(result.error.message)
  }
}

function handleRevokeToken(token: Token) {
  showConfirm({
    title: "Revoke token",
    message: `Are you sure you want to revoke the token "${token.name}"?`,
    confirmText: "Revoke token",
    confirmColor: "error",
    onConfirm: async () => {
      const result = await tokenService.deleteToken(token.id)
      if (isSuccess(result)) {
        notification.showSuccess(`Token "${token.name}" revoked`)
        await loadTokens()
      } else {
        notification.showError(`Failed to revoke token "${token.name}": ${result.error.message}`)
      }
    }
  })
}
</script>

<style scoped>
.token-dates {
  display: block;
  color: rgb(var(--v-theme-on-surface-variant));
}

.section-content {
  padding: 24px;
}

.section-content code {
  background: rgba(var(--v-theme-primary), 0.08);
  color: rgb(var(--v-theme-primary));
  padding: 2px 6px;
  border-radius: var(--kellnr-radius);
  font-size: 13px;
  font-family: var(--kellnr-font-mono);
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
  border-radius: var(--kellnr-radius);
  background: rgb(var(--v-theme-surface));
}

/* Token Created Display */
.token-created {
  width: 100%;
  min-width: 0;
}

.token-code {
  margin: 8px 0 16px;
}

.token-code:last-child {
  margin-bottom: 0;
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
}
</style>
