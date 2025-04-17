<template>
  <v-container>
    <h2 class="text-h4 mb-4">Change Password</h2>
    <v-form @submit.prevent="changePwd">
      <v-text-field v-model="old_pwd" label="Old password" type="password" variant="outlined"
        prepend-inner-icon="mdi-lock" :rules="[rules.required]" class="mb-2"></v-text-field>

      <v-text-field v-model="new_pwd1" label="New password" type="password" variant="outlined"
        prepend-inner-icon="mdi-lock" :rules="[rules.required]" class="mb-2"></v-text-field>

      <v-text-field v-model="new_pwd2" label="Confirm new password" type="password" variant="outlined"
        prepend-inner-icon="mdi-lock" :rules="[rules.required, rules.passwordMatch]" class="mb-4"></v-text-field>

      <v-alert v-if="pwdChangeStatus" :type="pwdChangeStatus === 'Success' ? 'success' : 'error'" closable
        variant="tonal" @update:model-value="pwdChangeStatus = ''" class="mb-4">
        {{ pwdChangeMsg }}
      </v-alert>

      <v-btn color="primary" type="submit" :loading="loading">
        Apply
      </v-btn>
    </v-form>
  </v-container>
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
