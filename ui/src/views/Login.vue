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
            <v-form ref="form" @submit.prevent="handleLogin" v-model="isFormValid">
              <div class="input-group">
                <label class="input-label">Username</label>
                <v-text-field
                  v-model="username"
                  placeholder="Enter your username"
                  prepend-inner-icon="mdi-account-outline"
                  variant="outlined"
                  :rules="usernameRules"
                  required
                  density="comfortable"
                  class="login-input"
                  rounded="lg"
                />
              </div>

              <div class="input-group">
                <label class="input-label">Password</label>
                <v-text-field
                  v-model="password"
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
                v-if="status.hasStatus"
                :type="status.isSuccess ? 'success' : 'error'"
                class="mt-4"
                density="compact"
                variant="tonal"
                closable
                rounded="lg"
                @click:close="status.clear()"
              >
                {{ status.message }}
              </v-alert>

              <v-btn
                color="primary"
                size="large"
                type="submit"
                block
                :disabled="!isFormValid"
                :loading="loading"
                class="login-button mt-6"
                rounded="lg"
              >
                <v-icon icon="mdi-login" class="mr-2" />
                Sign In
              </v-btn>
            </v-form>

            <!-- OAuth2/SSO Login -->
            <template v-if="oauth2Enabled">
              <div class="oauth2-divider">
                <span>or</span>
              </div>

              <v-btn
                color="secondary"
                size="large"
                variant="outlined"
                block
                class="oauth2-button"
                rounded="lg"
                @click="handleOAuth2Login"
              >
                <v-icon icon="mdi-shield-account" class="mr-2" />
                {{ oauth2ButtonText }}
              </v-btn>
            </template>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue"
import { useStore } from "../store/store"
import { useStatusMessage } from "../composables"
import { userService, settingsService } from "../services"
import { isSuccess } from "../services/api"
import router from "../router"
import { OAUTH2_LOGIN } from "../remote-routes"

// State
const form = ref(null)
const isFormValid = ref(false)
const loading = ref(false)
const username = ref("")
const password = ref("")
const store = useStore()

// OAuth2 state
const oauth2Enabled = ref(false)
const oauth2ButtonText = ref("Login with SSO")

// Composables
const status = useStatusMessage()

// Validation rules
const usernameRules = [
  (v: string) => !!v || 'Username is required',
]

const passwordRules = [
  (v: string) => !!v || 'Password is required',
]

// Lifecycle
onMounted(async () => {
  if (store.rememberMe && store.rememberMeUser !== null) {
    username.value = store.rememberMeUser
  }

  // Check for OAuth2 error in query params (from error callback)
  const errorParam = router.currentRoute.value.query["error"]
  if (errorParam) {
    const errorMessage = Array.isArray(errorParam) ? errorParam[0] : errorParam
    if (errorMessage) {
      status.setError(errorMessage)
    }
  }

  // Fetch OAuth2 configuration
  const oauth2Result = await settingsService.getOAuth2Config()
  if (isSuccess(oauth2Result)) {
    oauth2Enabled.value = oauth2Result.data.enabled
    oauth2ButtonText.value = oauth2Result.data.button_text
  }
})

// Handle OAuth2 login
function handleOAuth2Login() {
  // Redirect to OAuth2 login endpoint
  window.location.href = OAUTH2_LOGIN
}

// Handle login
async function handleLogin() {
  if (!isFormValid.value) {
    return
  }

  loading.value = true
  status.clear()

  const result = await userService.login({
    user: username.value,
    pwd: password.value,
    remember_me: store.rememberMe
  })

  loading.value = false

  if (isSuccess(result)) {
    status.setSuccess("Login successful")
    store.login(result.data)

    if (store.rememberMe) {
      store.rememberMeUser = username.value
    }

    // Redirect based on query parameter flag
    const redirectParam = router.currentRoute.value.query["redirect"]
    // Handle both string and array cases from Vue Router
    const redirect = Array.isArray(redirectParam) ? redirectParam[0] : redirectParam

    if (redirect === "me") {
      // From /me route (cargo login flow) - go to settings tokens tab
      router.push("/settings?tab=tokens")
    } else if (redirect === "settings") {
      router.push("/settings")
    } else {
      router.push("/")
    }
  } else {
    status.setError(result.error.message)
  }
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

.oauth2-divider {
  display: flex;
  align-items: center;
  text-align: center;
  margin: 20px 0;
}

.oauth2-divider::before,
.oauth2-divider::after {
  content: '';
  flex: 1;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.oauth2-divider span {
  padding: 0 16px;
  color: rgb(var(--v-theme-on-surface-variant));
  font-size: 0.875rem;
}

.oauth2-button {
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
