<template>
  <div>
    <!-- Login button for logged out users -->
    <v-btn v-if="store.loggedIn === false" :to="'/login'" variant="outlined" color="primary" density="comfortable"
      prepend-icon="mdi-login" :loading="isLoading" :disabled="isLoading">
      Log in
    </v-btn>

    <!-- Logout button for logged in users -->
    <v-btn v-else variant="outlined" color="primary" density="comfortable" prepend-icon="mdi-logout" @click="logOut()"
      :loading="isLoading" :disabled="isLoading">
      <span class="d-none d-sm-inline">{{ store.loggedInUser }}</span>
      <span class="d-sm-none">Logout</span>
    </v-btn>

    <!-- Snackbar for notifications -->
    <v-snackbar v-model="showSnackbar" :color="snackbarColor" :timeout="3000" location="bottom" class="mb-4">
      {{ snackbarText }}
      <template v-slot:actions>
        <v-btn variant="text" icon="mdi-close" @click="showSnackbar = false" size="small"></v-btn>
      </template>
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref } from 'vue'
import { useRouter } from "vue-router";
import { useStore } from "../store/store"
import { userService } from "../services";
import { isSuccess } from "../services/api";

const router = useRouter()
const store = useStore()

// Loading state
const isLoading = ref(false)

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

async function logOut() {
  isLoading.value = true

  try {
    const result = await userService.logout()
    if (isSuccess(result)) {
      store.logout()
      router.push("/")
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

  try {
    const result = await userService.getLoginState()
    if (isSuccess(result) && result.data) {
      if (result.data.is_logged_in) {
        store.login(result.data)
        showNotification(`Welcome back, ${store.loggedInUser}!`)
      } else {
        store.logout()
      }
    }
  } finally {
    isLoading.value = false
  }
}
</script>
