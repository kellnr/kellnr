<template>
  <div>
    <!-- Login button for logged out users, not needed on the login page itself.
         The component stays mounted there, it shows the welcome message. -->
    <v-btn v-if="store.loggedIn === false && route.name !== 'Login'" :to="'/login'" variant="outlined" color="primary"
      density="comfortable" prepend-icon="mdi-login" :loading="isLoading" :disabled="isLoading"
      data-testid="login-button">
      Log in
    </v-btn>

    <!-- User menu for logged in users -->
    <v-menu v-else-if="store.loggedIn !== false" location="bottom end">
      <template v-slot:activator="{ props }">
        <v-btn v-bind="props" variant="outlined" color="primary" density="comfortable"
          :loading="isLoading" :disabled="isLoading" :aria-label="`User menu for ${store.loggedInUser}`"
          data-testid="user-menu-button">
          <v-icon icon="mdi-account-circle" class="me-sm-2"></v-icon>
          <span class="d-none d-sm-inline">{{ store.loggedInUser }}</span>
          <v-icon icon="mdi-menu-down" class="d-none d-lg-inline ms-1"></v-icon>
        </v-btn>
      </template>

      <v-list density="compact" min-width="180" data-testid="user-menu">
        <v-list-item class="d-sm-none" :title="store.loggedInUser ?? ''" subtitle="Logged in"></v-list-item>
        <v-divider class="d-sm-none"></v-divider>
        <v-list-item to="/settings" prepend-icon="mdi-cog" title="Settings" data-testid="user-menu-settings"></v-list-item>
        <v-list-item prepend-icon="mdi-logout" title="Log out" @click="logOut()" data-testid="logout-button"></v-list-item>
      </v-list>
    </v-menu>

    <!-- Snackbar for notifications -->
    <v-snackbar v-model="showSnackbar" :color="snackbarColor" :timeout="3000" location="bottom" class="mb-4" data-testid="snackbar">
      {{ snackbarText }}
      <template v-slot:actions>
        <v-btn variant="text" icon="mdi-close" @click="showSnackbar = false" size="small"></v-btn>
      </template>
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref, watch } from 'vue'
import { useRoute, useRouter } from "vue-router";
import { useStore } from "../store/store"
import { userService } from "../services";
import { isSuccess } from "../services/api";
import { takeSsoLoginPending } from "../common/ssoLogin";

const router = useRouter()
const route = useRoute()
const store = useStore()

// Loading state
const isLoading = ref(false)

// True while the session state is restored on page load. A user set during
// that check was already logged in, so no welcome message is shown.
const checkingLoginStatus = ref(false)

// Snackbar state
const showSnackbar = ref(false)
const snackbarText = ref('')
const snackbarColor = ref('success')

// Show notification snackbar
function showNotification(message: string, isError: boolean = false) {
  snackbarText.value = message
  snackbarColor.value = isError ? 'error' : 'success'
  showSnackbar.value = true
}

onBeforeMount(() => {
  getLoginStatus()
})

// Greet the user once, right after logging in on the login page.
watch(() => store.loggedInUser, (user, previousUser) => {
  if (user && !previousUser && !checkingLoginStatus.value) {
    showNotification(`Welcome back, ${user}!`)
  }
}, { flush: 'sync' })

async function logOut() {
  isLoading.value = true

  try {
    const result = await userService.logout()
    if (isSuccess(result)) {
      store.logout()
      if (result.data?.logout_url) {
        window.location.href = result.data.logout_url
        return
      }
      router.push({ name: 'Landing', query: { from: 'logout' } })
      showNotification("Successfully logged out")
    } else {
      showNotification(result.error || "Logout failed", true)
    }
  } finally {
    isLoading.value = false
  }
}

async function getLoginStatus() {
  isLoading.value = true
  checkingLoginStatus.value = true
  // An SSO login returns with a page load, so greet the user here
  const ssoLoginPending = takeSsoLoginPending()

  try {
    const result = await userService.getLoginState()
    if (isSuccess(result) && result.data) {
      if (result.data.is_logged_in) {
        store.login(result.data)
        if (ssoLoginPending) {
          showNotification(`Welcome back, ${store.loggedInUser}!`)
        }
      } else {
        store.logout()
      }
    }
  } finally {
    checkingLoginStatus.value = false
    isLoading.value = false
  }
}
</script>
