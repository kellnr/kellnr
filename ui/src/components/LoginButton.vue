<template>
  <router-link v-if="store.loggedIn === false" class="k-button" to="/login">
    <span class="icon">
      <i class="fas fa-sign-in-alt"></i>
    </span>
    Log in
  </router-link>

  <a v-else class="k-button" @click="logOut()">
    <span class="icon">
      <i class="fas fa-sign-out-alt"></i>
    </span>
    {{ store.loggedInUser }}
  </a>
</template>

<script setup lang="ts">
import { onBeforeMount } from 'vue'
import axios from "axios";
import { useRouter } from "vue-router";
import { useStore } from "../store/store"
import { LOGIN_STATE, LOGOUT } from "../remote-routes";

const router = useRouter()
const store = useStore()

onBeforeMount(() => {
  getLoginStatus()
})

function logOut() {
  axios
    .get(LOGOUT)
    .then((res) => {
      if (res.status == 200) {
        store.logout();
        router.push("/");
      }
    })
    .catch((error) => {
      if (error.response) {
        if (error.response.status == 500) {
          console.log("Logout failed: Internal server error");
        } else {
          console.log("Logout failed: Unknown error");
        }
      }
    });
}

function getLoginStatus() {
  axios
    .get(LOGIN_STATE)
    .then((res) => {
      if (res.status == 200) {
        if (res.data.is_logged_in) {
          store.login(res.data)
        } else {
          store.logout()
        }
      }
    })
    .catch((error) => {
      if (error.response) {
        console.log("Failed to get login status");
      }
    });
}
</script>

<style></style>
