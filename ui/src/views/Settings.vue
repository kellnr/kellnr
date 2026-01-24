<template>
  <v-container fluid class="pa-4 pa-md-6">
    <v-row>
      <!-- Left sidebar -->
      <v-col cols="12" md="4" lg="3">
        <v-card class="sidebar-card" elevation="0">
          <div class="sidebar-header">
            <v-icon icon="mdi-cog" size="small" color="primary" class="me-3"></v-icon>
            <span class="text-h6 font-weight-bold">Settings</span>
          </div>

          <v-list nav class="pa-2">
            <v-list-item
              @click="activeTab = 'password'"
              :active="activeTab === 'password'"
              color="primary"
              rounded="lg"
              class="mb-1"
            >
              <template v-slot:prepend>
                <div class="nav-icon-wrapper" :class="{ active: activeTab === 'password' }">
                  <v-icon icon="mdi-key" size="small"></v-icon>
                </div>
              </template>
              <v-list-item-title class="text-body-1 font-weight-medium">Change Password</v-list-item-title>
              <template v-slot:append>
                <v-icon icon="mdi-chevron-right" size="small" class="nav-chevron"></v-icon>
              </template>
            </v-list-item>

            <v-list-item
              @click="activeTab = 'tokens'"
              :active="activeTab === 'tokens'"
              color="primary"
              rounded="lg"
              class="mb-1"
            >
              <template v-slot:prepend>
                <div class="nav-icon-wrapper" :class="{ active: activeTab === 'tokens' }">
                  <v-icon icon="mdi-shield-key" size="small"></v-icon>
                </div>
              </template>
              <v-list-item-title class="text-body-1 font-weight-medium">Authentication Tokens</v-list-item-title>
              <template v-slot:append>
                <v-icon icon="mdi-chevron-right" size="small" class="nav-chevron"></v-icon>
              </template>
            </v-list-item>

            <template v-if="store.loggedInUserIsAdmin">
              <v-list-subheader class="text-overline mt-2">Administration</v-list-subheader>

              <v-list-item
                @click="activeTab = 'users'"
                :active="activeTab === 'users'"
                color="primary"
                rounded="lg"
                class="mb-1"
              >
                <template v-slot:prepend>
                  <div class="nav-icon-wrapper" :class="{ active: activeTab === 'users' }">
                    <v-icon icon="mdi-account-multiple" size="small"></v-icon>
                  </div>
                </template>
                <v-list-item-title class="text-body-1 font-weight-medium">User Management</v-list-item-title>
                <template v-slot:append>
                  <v-icon icon="mdi-chevron-right" size="small" class="nav-chevron"></v-icon>
                </template>
              </v-list-item>

              <v-list-item
                @click="activeTab = 'groups'"
                :active="activeTab === 'groups'"
                color="primary"
                rounded="lg"
                class="mb-1"
              >
                <template v-slot:prepend>
                  <div class="nav-icon-wrapper" :class="{ active: activeTab === 'groups' }">
                    <v-icon icon="mdi-account-group" size="small"></v-icon>
                  </div>
                </template>
                <v-list-item-title class="text-body-1 font-weight-medium">Group Management</v-list-item-title>
                <template v-slot:append>
                  <v-icon icon="mdi-chevron-right" size="small" class="nav-chevron"></v-icon>
                </template>
              </v-list-item>

              <v-list-item
                @click="activeTab = 'config'"
                :active="activeTab === 'config'"
                color="primary"
                rounded="lg"
              >
                <template v-slot:prepend>
                  <div class="nav-icon-wrapper" :class="{ active: activeTab === 'config' }">
                    <v-icon icon="mdi-tune" size="small"></v-icon>
                  </div>
                </template>
                <v-list-item-title class="text-body-1 font-weight-medium">Startup Config</v-list-item-title>
                <template v-slot:append>
                  <v-icon icon="mdi-chevron-right" size="small" class="nav-chevron"></v-icon>
                </template>
              </v-list-item>
            </template>
          </v-list>
        </v-card>
      </v-col>

      <!-- Content area -->
      <v-col cols="12" md="8" lg="9">
        <v-card class="content-card" elevation="0">
          <change-password v-if="activeTab === 'password'" />
          <auth-token v-if="activeTab === 'tokens'" />
          <user-mgmt v-if="activeTab === 'users'" />
          <group-mgmt v-if="activeTab === 'groups'" />
          <startup-config v-if="activeTab === 'config'" />
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
import { useRoute } from "vue-router";

type SettingsTab = 'password' | 'tokens' | 'users' | 'groups' | 'config'

const route = useRoute()

function getInitialTab(): SettingsTab {
  const tab = route.query.tab as string | undefined
  const validTabs: SettingsTab[] = ['password', 'tokens', 'users', 'groups', 'config']
  return validTabs.includes(tab as SettingsTab) ? (tab as SettingsTab) : 'password'
}

const activeTab = ref<SettingsTab>(getInitialTab())
const store = useStore()
</script>

<style scoped>
/* Sidebar Card */
.sidebar-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 12px;
  overflow: hidden;
}

.sidebar-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  background: rgba(var(--v-theme-primary), 0.05);
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

/* Nav Icon Wrapper */
.nav-icon-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background: rgba(var(--v-theme-primary), 0.08);
  color: rgb(var(--v-theme-primary));
  margin-right: 4px;
  transition: all 0.2s ease;
}

.nav-icon-wrapper.active {
  background: rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-primary));
}

/* Nav Chevron */
.nav-chevron {
  opacity: 0.3;
  transition: opacity 0.2s ease;
}

.v-list-item:hover .nav-chevron {
  opacity: 0.7;
}

.v-list-item--active .nav-chevron {
  opacity: 1;
}

/* Content Card */
.content-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 12px;
  min-height: 400px;
  overflow: hidden;
}

/* Responsive */
@media (max-width: 960px) {
  .sidebar-card {
    margin-bottom: 16px;
  }
}
</style>
