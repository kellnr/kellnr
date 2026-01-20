<template>
  <div>
    <!-- Header -->
    <div class="section-header">
      <v-icon icon="mdi-key" size="small" color="primary" class="me-3"></v-icon>
      <span class="text-h6 font-weight-bold">Change Password</span>
    </div>

    <!-- Form -->
    <div class="form-container">
      <p class="text-body-2 text-medium-emphasis mb-6">
        Update your password to keep your account secure. Choose a strong password that you don't use elsewhere.
      </p>

      <v-form @submit.prevent="handleChangePassword">
        <div class="mb-4">
          <label class="text-body-2 font-weight-medium d-block mb-2">Current Password</label>
          <v-text-field
            v-model="oldPassword"
            type="password"
            variant="outlined"
            density="comfortable"
            prepend-inner-icon="mdi-lock-outline"
            placeholder="Enter your current password"
            :rules="[rules.required]"
            hide-details="auto"
          ></v-text-field>
        </div>

        <v-divider class="my-5"></v-divider>
        <p class="text-overline text-medium-emphasis mb-4">New Password</p>

        <div class="mb-4">
          <label class="text-body-2 font-weight-medium d-block mb-2">New Password</label>
          <v-text-field
            v-model="newPassword1"
            type="password"
            variant="outlined"
            density="comfortable"
            prepend-inner-icon="mdi-lock-plus-outline"
            placeholder="Enter your new password"
            :rules="[rules.required]"
            hide-details="auto"
          ></v-text-field>
        </div>

        <div class="mb-5">
          <label class="text-body-2 font-weight-medium d-block mb-2">Confirm New Password</label>
          <v-text-field
            v-model="newPassword2"
            type="password"
            variant="outlined"
            density="comfortable"
            prepend-inner-icon="mdi-lock-check-outline"
            placeholder="Confirm your new password"
            :rules="[rules.required, rules.passwordMatch]"
            hide-details="auto"
          ></v-text-field>
        </div>

        <v-alert
          v-if="status.hasStatus"
          :type="status.isSuccess ? 'success' : 'error'"
          closable
          variant="tonal"
          @click:close="status.clear()"
          class="mb-5"
        >
          {{ status.message }}
        </v-alert>

        <v-divider class="mb-5"></v-divider>

        <v-btn
          color="primary"
          type="submit"
          :loading="loading"
          size="large"
        >
          <v-icon icon="mdi-check" size="small" class="me-2"></v-icon>
          Update Password
        </v-btn>
      </v-form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue"
import { useStatusMessage } from "../composables"
import { userService } from "../services"
import { isSuccess } from "../services/api"

// State
const loading = ref(false)
const oldPassword = ref("")
const newPassword1 = ref("")
const newPassword2 = ref("")

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
    // Reset form on success
    oldPassword.value = ""
    newPassword1.value = ""
    newPassword2.value = ""
  } else {
    status.setError(result.error.message)
  }
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

.form-container {
  padding: 24px;
  max-width: 500px;
}

:deep(.v-field) {
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
}

:deep(.v-field--focused) {
  background: rgb(var(--v-theme-surface));
}

/* Labels */
label {
  color: rgb(var(--v-theme-on-surface));
}

/* Dividers */
:deep(.v-divider) {
  opacity: 0.5;
}
</style>
