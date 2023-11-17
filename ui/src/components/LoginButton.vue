<template>
  <router-link
      v-if="store.state.loggedIn === false"
      class="k-button"
      to="/login"
  >
    <span class="icon">
      <i class="fas fa-sign-in-alt"></i>
    </span>
    Log in
  </router-link>

  <a v-else class="k-button" @click="logOut()">
        <span class="icon">
          <i class="fas fa-sign-out-alt"></i>
        </span>
    {{ store.state.loggedInUser }}
  </a>
</template>

<script setup lang="ts">
import {onBeforeMount} from 'vue'
import {MutationTypes} from "../store/mutation-types";
import axios from "axios";
import {useRouter} from "vue-router";
import {store} from "../store/store"
import {LOGIN_STATE, LOGOUT} from "../remote-routes";

const router = useRouter()

onBeforeMount(() => {
  getLoginStatus()
})

function logOut() {
  axios
      .get(LOGOUT)
      .then((res) => {
        if (res.status == 200) {
          store.commit(MutationTypes.LOGOUT, null);
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
            store.commit(MutationTypes.LOGIN, res.data);
          } else {
            store.commit(MutationTypes.LOGOUT, null);
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

<style>

</style>
