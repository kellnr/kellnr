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
import axios from "axios";
import { useRouter } from "vue-router";
import { useStore } from "../store/store"
import { LOGIN_STATE, LOGOUT } from "../remote-routes";
import { useDisplay } from "vuetify";

const router = useRouter()
const store = useStore()
const display = useDisplay()

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

function logOut() {
  isLoading.value = true

  axios
    .get(LOGOUT)
    .then((res) => {
      if (res.status == 200) {
        store.logout();
        router.push("/");
        showNotification("Successfully logged out")
      }
    })
    .catch((error) => {
      let errorMessage = "Logout failed: Unknown error"

      if (error.response) {
        if (error.response.status == 500) {
          errorMessage = "Logout failed: Internal server error"
        }
      }

      console.log(errorMessage)
      showNotification(errorMessage, true)
    })
    .finally(() => {
      isLoading.value = false
    });
}

function getLoginStatus() {
  isLoading.value = true

  axios
    .get(LOGIN_STATE)
    .then((res) => {
      if (res.status == 200) {
        if (res.data.is_logged_in) {
          store.login(res.data)
          showNotification(`Welcome back, ${store.loggedInUser}!`)
        } else {
          store.logout()
        }
      }
    })
    .catch((error) => {
      if (error.response) {
        const errorMessage = "Failed to get login status"
        console.log(errorMessage)
        showNotification(errorMessage, true)
      }
    })
    .finally(() => {
      isLoading.value = false
    });
}
</script>
