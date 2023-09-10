<template>
  <div id="settingsContainer">
    <div id="settingsNames" class="glass">
      <h1 class="k-h1">Settings</h1>
      <div @click="clickShowChangePwd" class="settingName">Change Password</div>
      <div @click="clickShowAuthToken" class="settingName">
        Authentication Tokens
      </div>
    </div>
    <div id="settings" class="glass">
      <div v-if="showChangePwd" class="setting">
        <change-password></change-password>
      </div>
      <div v-if="showAuthToken" class="setting">
        <auth-token></auth-token>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import ChangePassword from "../components/ChangePassword.vue";
import AuthToken from "../components/AuthToken.vue";
import {ref} from "vue";

const showChangePwd = ref(true)
const showAuthToken = ref(false)

function showNothing() {
  showChangePwd.value = false;
  showAuthToken.value = false;
}

function clickShowChangePwd() {
  showNothing();
  showChangePwd.value = true;
}

function clickShowAuthToken() {
  showNothing();
  showAuthToken.value = true;
}
</script>

<style scoped>
#settingsContainer {
  display: grid;
  grid-template-columns: max-content auto;
}

#settingsNames {
  display: grid;
  grid-template-rows: max-content max-content max-content max-content max-content;
  margin-right: 3em;
  height: fit-content;
}

body[color-theme="dark"] .settingName {
  color: var(--dark-color);
}

.settingName {
  padding-bottom: 0.5em;
}

.settingName:hover {
  text-decoration: underline;
  cursor: pointer;
}

</style>
