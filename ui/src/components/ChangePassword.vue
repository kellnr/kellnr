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

      <v-form @submit.prevent="changePwd">
        <div class="mb-4">
          <label class="text-body-2 font-weight-medium d-block mb-2">Current Password</label>
          <v-text-field
            v-model="old_pwd"
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
            v-model="new_pwd1"
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
            v-model="new_pwd2"
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
          v-if="pwdChangeStatus"
          :type="pwdChangeStatus === 'Success' ? 'success' : 'error'"
          closable
          variant="tonal"
          @update:model-value="pwdChangeStatus = ''"
          class="mb-5"
        >
          {{ pwdChangeMsg }}
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
import { ref } from "vue";
import axios from "axios";
import { CHANGE_PWD } from "../remote-routes";

const loading = ref(false);
const pwdChangeStatus = ref("");
const pwdChangeMsg = ref("");
const old_pwd = ref("");
const new_pwd1 = ref("");
const new_pwd2 = ref("");

// Simplified validation rules
const rules = {
  required: (v: string) => !!v || 'Field is required',
  passwordMatch: (v: string) => v === new_pwd1.value || 'Passwords do not match'
};

function changePwd() {
  // Check if passwords match before sending to server
  if (new_pwd1.value !== new_pwd2.value) {
    pwdChangeStatus.value = "Error";
    pwdChangeMsg.value = "New passwords do not match";
    return;
  }

  loading.value = true;
  const postData = {
    old_pwd: old_pwd.value,
    new_pwd1: new_pwd1.value,
    new_pwd2: new_pwd2.value,
  };

  axios
    .post(CHANGE_PWD, postData)
    .then((res) => {
      if (res.status == 200) {
        pwdChangeMsg.value = "Password changed successfully";
        pwdChangeStatus.value = "Success";
        // Reset form on success
        old_pwd.value = "";
        new_pwd1.value = "";
        new_pwd2.value = "";
      }
    })
    .catch((error) => {
      pwdChangeStatus.value = "Error";
      if (error.response) {
        if (error.response.status == 400) {
          pwdChangeMsg.value = "Old password is incorrect or passwords do not match";
        } else if (error.response.status == 404) {
          pwdChangeMsg.value = "Unauthorized. Please login first.";
        } else if (error.response.status == 500) {
          pwdChangeMsg.value = "Internal server error";
        } else {
          pwdChangeMsg.value = "Unknown error";
        }
      } else {
        pwdChangeMsg.value = "Network error. Please try again.";
      }
    })
    .finally(() => {
      loading.value = false;
    });
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
