<template>
  <div>
    <SectionHeader icon="mdi-key" :title="SETTINGS_SECTION_TITLES.password" role="heading" aria-level="2" />

    <div class="section-content">
      <p class="text-body-medium text-medium-emphasis mb-5">
        Update your password to keep your account secure. Choose a strong password that you don't use elsewhere.
      </p>

      <FormSection icon="mdi-lock-reset" title="Password">
        <v-form ref="form" @submit.prevent="handleChangePassword">
          <!-- Tells password managers which account the new password belongs to -->
          <input type="text" name="username" autocomplete="username" :value="store.loggedInUser ?? ''" readonly hidden />

          <div class="form-grid">
            <div class="form-field">
              <label class="field-label" for="current-password">Current password</label>
              <v-text-field
                id="current-password"
                v-model="oldPassword"
                type="password"
                autocomplete="current-password"
                variant="outlined"
                density="comfortable"
                prepend-inner-icon="mdi-lock-outline"
                placeholder="Current password"
                :rules="[rules.required]"
                hide-details="auto"
              ></v-text-field>
            </div>

            <div class="form-field">
              <label class="field-label" for="new-password">New password</label>
              <v-text-field
                id="new-password"
                v-model="newPassword1"
                type="password"
                autocomplete="new-password"
                variant="outlined"
                density="comfortable"
                prepend-inner-icon="mdi-lock-plus-outline"
                placeholder="New password"
                :rules="[rules.required]"
                hide-details="auto"
              ></v-text-field>
            </div>

            <div class="form-field">
              <label class="field-label" for="new-password-confirm">Confirm new password</label>
              <v-text-field
                id="new-password-confirm"
                v-model="newPassword2"
                type="password"
                autocomplete="new-password"
                variant="outlined"
                density="comfortable"
                prepend-inner-icon="mdi-lock-check-outline"
                placeholder="Repeat new password"
                :rules="[rules.required, rules.passwordMatch]"
                hide-details="auto"
              ></v-text-field>
            </div>
          </div>

          <v-alert
            v-if="status.hasStatus"
            :type="status.isSuccess ? 'success' : 'error'"
            closable
            variant="tonal"
            @click:close="status.clear()"
            class="mb-4"
          >
            {{ status.message }}
          </v-alert>

          <div class="form-actions">
            <v-btn
              color="primary"
              type="submit"
              :loading="loading"
              size="large"
            >
              <v-icon icon="mdi-check" size="small" class="me-2"></v-icon>
              Update password
            </v-btn>
          </div>
        </v-form>
      </FormSection>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue"
import { useStatusMessage } from "../composables"
import { userService } from "../services"
import { isSuccess } from "../services/api"
import { SETTINGS_SECTION_TITLES } from "../common/settingsSections"
import { useStore } from "../store/store"
import { SectionHeader, FormSection } from "./common"

// State
const store = useStore()
const loading = ref(false)
const oldPassword = ref("")
const newPassword1 = ref("")
const newPassword2 = ref("")
const form = ref<InstanceType<typeof import('vuetify/components').VForm> | null>(null)

// Composables
const status = useStatusMessage()

// Validation rules
const rules = {
  required: (v: string) => !!v || 'Field is required',
  passwordMatch: (v: string) => v === newPassword1.value || 'Passwords do not match'
}

// Handle password change
async function handleChangePassword() {
  // Check if passwords match before sending to server
  if (newPassword1.value !== newPassword2.value) {
    status.setError("New passwords do not match")
    return
  }

  loading.value = true
  status.clear()

  const result = await userService.changePassword(
    oldPassword.value,
    newPassword1.value,
    newPassword2.value
  )

  loading.value = false

  if (isSuccess(result)) {
    status.setSuccess("Password changed successfully")
    form.value?.reset()
  } else {
    status.setError(result.error.message)
  }
}
</script>

<style scoped>
/* Same layout as the add user form in the user management */
.section-content {
  padding: 24px;
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

.form-actions {
  padding-top: 16px;
  border-top: 1px solid rgb(var(--v-theme-outline));
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
