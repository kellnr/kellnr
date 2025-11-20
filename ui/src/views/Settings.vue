<template>
  <v-container fluid class="pa-0">
    <v-row>
      <!-- Left sidebar with increased width -->
      <v-col cols="12" md="4" lg="3">
        <v-card class="mb-4">
          <v-card-title class="text-h5 font-weight-bold">
            Settings
          </v-card-title>

          <v-divider></v-divider>

          <v-list nav>
            <v-list-item @click="clickShowChangePwd" :active="showChangePwd" color="primary" class="py-2">
              <template v-slot:prepend>
                <v-icon>mdi-key</v-icon>
              </template>
              <v-list-item-title>Change Password</v-list-item-title>
            </v-list-item>

            <v-list-item @click="clickShowAuthToken" :active="showAuthToken" color="primary" class="py-2">
              <template v-slot:prepend>
                <v-icon>mdi-shield-key</v-icon>
              </template>
              <v-list-item-title>Authentication Tokens</v-list-item-title>
            </v-list-item>

            <v-list-item v-if="store.loggedInUserIsAdmin" @click="clickShowUserMgmt" :active="showUserMgmt"
              color="primary" class="py-2">
              <template v-slot:prepend>
                <v-icon>mdi-account-multiple</v-icon>
              </template>
              <v-list-item-title>User Management</v-list-item-title>
            </v-list-item>

            <v-list-item v-if="store.loggedInUserIsAdmin" @click="clickShowGroupMgmt" :active="showGroupMgmt"
              color="primary" class="py-2">
              <template v-slot:prepend>
                <v-icon>mdi-account-group</v-icon>
              </template>
              <v-list-item-title>Group Management</v-list-item-title>
            </v-list-item>

            <v-list-item v-if="store.loggedInUserIsAdmin" @click="clickShowStartupConfig" :active="showStartupConfig"
              color="primary" class="py-2">
              <template v-slot:prepend>
                <v-icon>mdi-cog</v-icon>
              </template>
              <v-list-item-title>Startup Config</v-list-item-title>
            </v-list-item>
          </v-list>
        </v-card>
      </v-col>

      <!-- Content area with adjusted width -->
      <v-col cols="12" md="8" lg="9">
        <v-card>
          <v-card-text class="pa-4">
            <!-- Change Password Section -->
            <div v-if="showChangePwd">
              <change-password></change-password>
            </div>

            <!-- Auth Token Section -->
            <div v-if="showAuthToken">
              <auth-token></auth-token>
            </div>

            <!-- User Management Section -->
            <div v-if="showUserMgmt">
              <user-mgmt></user-mgmt>
            </div>

            <!-- Group Management Section -->
            <div v-if="showGroupMgmt">
              <group-mgmt></group-mgmt>
            </div>

            <!-- Startup Config Section -->
            <div v-if="showStartupConfig">
              <startup-config></startup-config>
            </div>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
import ChangePassword from "../components/ChangePassword.vue";
import AuthToken from "../components/AuthToken.vue";
import UserMgmt from "../components/UserMgmt.vue";
import GroupMgmt from "../components/GroupMgmt.vue";
import StartupConfig from "../components/StartupConfig.vue";
import { useStore } from "../store/store";
import { ref } from "vue";

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
/* Force word wrapping on v-list-item-title to ensure they don't overflow */
:deep(.v-list-item-title) {
  white-space: normal;
  word-break: break-word;
}

/* Ensure the list items have enough height for the wrapped text */
:deep(.v-list-item) {
  min-height: 48px;
}
</style>
