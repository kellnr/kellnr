<template>
  <div id="settingsContainer">
    <div id="settingsNames" class="glass">
      <h1 class="k-h1">Settings</h1>
      <div @click="clickShowChangePwd" class="settingName clickable">Change Password</div>
      <div @click="clickShowAuthToken" class="settingName clickable">
        Authentication Tokens
      </div>
      <div v-if="store.loggedInUserIsAdmin" @click="clickShowUserMgmt" class="settingName clickable">User Management</div>
      <div v-if="store.loggedInUserIsAdmin" @click="clickShowGroupMgmt" class="settingName clickable">Group Management</div>
      <div v-if="store.loggedInUserIsAdmin" @click="clickShowStartupConfig" class="settingName clickable">
        Startup Config
      </div>
    </div>
    <div id="settings" class="glass">
      <div v-if="showChangePwd" class="setting">
        <change-password></change-password>
      </div>
      <div v-if="showAuthToken" class="setting">
        <auth-token></auth-token>
      </div>
      <div v-if="showUserMgmt" class="setting">
        <user-mgmt></user-mgmt>
      </div>
      <div v-if="showGroupMgmt" class="setting">
        <group-mgmt></group-mgmt>
      </div>
      <div v-if="showStartupConfig" class="setting">
        <startup-config></startup-config>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import ChangePassword from "../components/ChangePassword.vue";
import AuthToken from "../components/AuthToken.vue";
import UserMgmt from "../components/UserMgmt.vue";
import GroupMgmt from "../components/GroupMgmt.vue";
import StartupConfig from "../components/StartupConfig.vue";
import {useStore} from "../store/store";
import {ref} from "vue";

const showChangePwd = ref(true)
const showAuthToken = ref(false)
const showUserMgmt = ref(false)
const showGroupMgmt = ref(false)
const showStartupConfig = ref(false)
const store = useStore()

function showNothing() {
  showChangePwd.value = false;
  showAuthToken.value = false;
  showUserMgmt.value = false;
  showGroupMgmt.value = false;
  showStartupConfig.value = false;
}

function clickShowChangePwd() {
  showNothing();
  showChangePwd.value = true;
}

function clickShowAuthToken() {
  showNothing();
  showAuthToken.value = true;
}

function clickShowUserMgmt() {
  showNothing();
  showUserMgmt.value = true;
}

function clickShowGroupMgmt() {
  showNothing();
  showGroupMgmt.value = true;
}

function clickShowStartupConfig() {
  showNothing();
  showStartupConfig.value = true;
}

</script>

<style scoped>
#settingsContainer {
  display: grid;
}

#settingsNames {
  display: grid;
  grid-template-rows: max-content max-content max-content max-content max-content max-content;
  margin-right: 3em;
  margin-bottom: 1em;
  height: fit-content;
}

#settings {
  height: fit-content;
}

.settingName {
  padding-bottom: 0.5em;
}

@media only screen and (max-width: 992px) {
  #settingsContainer {
    grid-template-rows: max-content auto;
  }

  #settingsNames {
    width: 100%;
  }
}

@media only screen and (min-width: 992px) {
  #settingsContainer {
    grid-template-columns: max-content auto;
  }
}
</style>
