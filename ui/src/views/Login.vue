<template>
  <v-container fluid class="login-container pa-4">
    <v-row justify="center">
      <v-col cols="12" sm="10" md="6" lg="4" xl="3" class="login-col">
        <v-card class="login-card" elevation="0" rounded="xl">
          <!-- Header -->
          <div class="login-header">
            <h1 class="login-title">Welcome Back</h1>
            <p class="login-subtitle">Sign in to your Kellnr account</p>
          </div>

          <!-- Form -->
          <v-card-text class="pa-6 pt-2">
            <v-form ref="form" @submit.prevent="submit" v-model="isFormValid">
              <div class="input-group">
                <label class="input-label">Username</label>
                <v-text-field
                  v-model="user"
                  placeholder="Enter your username"
                  prepend-inner-icon="mdi-account-outline"
                  variant="outlined"
                  :rules="userRules"
                  required
                  density="comfortable"
                  class="login-input"
                  rounded="lg"
                />
              </div>

              <div class="input-group">
                <label class="input-label">Password</label>
                <v-text-field
                  v-model="pwd"
                  placeholder="Enter your password"
                  prepend-inner-icon="mdi-lock-outline"
                  type="password"
                  variant="outlined"
                  :rules="passwordRules"
                  required
                  density="comfortable"
                  class="login-input"
                  rounded="lg"
                />
              </div>

              <v-checkbox
                v-model="store.rememberMe"
                label="Remember me"
                color="primary"
                hide-details
                class="remember-checkbox"
              />

              <v-alert
                v-if="loginStatus"
                :type="loginStatus === 'Success' ? 'success' : 'error'"
                class="mt-4"
                density="compact"
                variant="tonal"
                closable
                rounded="lg"
                @click:close="loginStatus = ''"
              >
                {{ loginStatusMsg }}
              </v-alert>

              <v-btn
                color="primary"
                size="large"
                type="submit"
                block
                :disabled="!isFormValid"
                class="login-button mt-6"
                rounded="lg"
              >
                <v-icon icon="mdi-login" class="mr-2" />
                Sign In
              </v-btn>
            </v-form>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import axios from "axios";
import { useStore } from "../store/store";
import { LOGIN } from "../remote-routes";
import router from "../router";

const form = ref(null);
const isFormValid = ref(false);
const loginStatusMsg = ref("");
const loginStatus = ref(""); // "", "Error", "Success"
const user = ref("");
const pwd = ref("");
const store = useStore();

// Validation rules
const userRules = [
  (v: string) => !!v || 'Username is required',
];

const passwordRules = [
  (v: string) => !!v || 'Password is required',
];

onMounted(() => {
  if (store.rememberMe && store.rememberMeUser !== null) {
    user.value = store.rememberMeUser;
  }
});

function submit() {
  if (!isFormValid.value) {
    return; // Don't submit if form is not valid
  }

  const postData = { user: user.value, pwd: pwd.value };
  axios
    .post(LOGIN, postData)
    .then((res) => {
      if (res.status == 200) {
        loginStatusMsg.value = "Login successful";
        loginStatus.value = "Success";
        store.login(res.data);
        if (store.rememberMe) {
          store.rememberMeUser = user.value;
        }
        if (router.currentRoute.value.query["redirect"] === "settings") {
          router.push("/settings");
        } else {
          router.push("/");
        }
      }
    })
    .catch((error) => {
      if (error.response) {
        loginStatus.value = "Error";
        if (error.response.status == 401) {
          loginStatusMsg.value = "Wrong user or password";
        } else if (error.response.status == 500) {
          loginStatusMsg.value = "Internal server error";
        } else {
          loginStatusMsg.value = "Unknown error";
        }
      }
    });
}
</script>

<style scoped>
.login-container {
  min-height: calc(100vh - 64px);
}

.login-col {
  margin-top: 32px;
}

.login-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  overflow: hidden;
}

.login-header {
  text-align: center;
  padding: 24px 24px 20px;
  background: rgba(var(--v-theme-primary), 0.03);
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.login-title {
  font-size: 1.5rem;
  font-weight: 600;
  color: rgb(var(--v-theme-on-surface));
  margin-bottom: 4px;
}

.login-subtitle {
  font-size: 0.9rem;
  color: rgb(var(--v-theme-on-surface-variant));
  margin: 0;
}

.input-group {
  margin-bottom: 16px;
}

.input-label {
  display: block;
  font-size: 0.875rem;
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface));
  margin-bottom: 6px;
}

.login-input :deep(.v-field) {
  background: rgb(var(--v-theme-surface));
}

.login-input :deep(.v-field__prepend-inner .v-icon) {
  color: rgb(var(--v-theme-primary));
  opacity: 0.7;
}

.login-input :deep(.v-field--focused .v-field__prepend-inner .v-icon) {
  opacity: 1;
}

.remember-checkbox {
  margin-top: 4px;
}

.remember-checkbox :deep(.v-label) {
  font-size: 0.875rem;
  color: rgb(var(--v-theme-on-surface-variant));
}

.login-button {
  font-weight: 600;
  text-transform: none;
  letter-spacing: 0.25px;
}

/* Responsive adjustments */
@media (max-width: 600px) {
  .login-header {
    padding: 20px 20px 16px;
  }

  .login-title {
    font-size: 1.35rem;
  }
}
</style>
