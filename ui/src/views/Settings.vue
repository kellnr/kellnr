<template>
    <v-container fluid class="pa-4 pa-md-6 settings-container">
        <!-- Mobile Navigation Header -->
        <div class="mobile-nav-header d-md-none mb-4">
            <v-card class="sidebar-card" elevation="0">
                <div class="sidebar-header" @click="mobileNavOpen = !mobileNavOpen" style="cursor: pointer;">
                    <v-icon icon="mdi-cog" size="small" color="primary" class="me-3"></v-icon>
                    <span class="text-h6 font-weight-bold">Settings</span>
                    <v-spacer></v-spacer>
                    <v-chip size="small" variant="tonal" color="primary" class="me-2">
                        {{ getTabLabel(activeTab) }}
                    </v-chip>
                    <v-icon :icon="mobileNavOpen ? 'mdi-chevron-up' : 'mdi-chevron-down'" size="small"></v-icon>
                </div>

                <v-expand-transition>
                    <v-list v-show="mobileNavOpen" nav class="pa-2">
                        <template v-for="item in visibleUserNavItems" :key="item.tab">
                            <v-list-item @click="selectTab(item.tab)" :active="activeTab === item.tab" color="primary"
                                rounded="lg" class="mb-1" :data-testid="item.mobileTestId">
                                <template v-slot:prepend>
                                    <div class="nav-icon-wrapper" :class="{ active: activeTab === item.tab }">
                                        <v-icon :icon="item.icon" size="small"></v-icon>
                                    </div>
                                </template>
                                <v-list-item-title class="text-body-1 font-weight-medium">{{ item.mobileLabel
                                    }}</v-list-item-title>
                            </v-list-item>
                        </template>

                        <template v-if="store.loggedInUserIsAdmin">
                            <v-list-subheader class="text-overline mt-2">Administration</v-list-subheader>

                            <template v-for="item in visibleAdminNavItems" :key="item.tab">
                                <v-list-item @click="selectTab(item.tab)" :active="activeTab === item.tab"
                                    color="primary" rounded="lg" :class="{ 'mb-1': !isLastAdminItem(item) }"
                                    :data-testid="item.mobileTestId">
                                    <template v-slot:prepend>
                                        <div class="nav-icon-wrapper" :class="{ active: activeTab === item.tab }">
                                            <v-icon :icon="item.icon" size="small"></v-icon>
                                        </div>
                                    </template>
                                    <v-list-item-title class="text-body-1 font-weight-medium">{{ item.mobileLabel
                                        }}</v-list-item-title>
                                </v-list-item>
                            </template>
                        </template>
                    </v-list>
                </v-expand-transition>
            </v-card>
        </div>

        <v-row>
            <!-- Desktop Left sidebar -->
            <v-col cols="12" md="4" lg="3" class="d-none d-md-block">
                <v-card class="sidebar-card" elevation="0">
                    <div class="sidebar-header">
                        <v-icon icon="mdi-cog" size="small" color="primary" class="me-3"></v-icon>
                        <span class="text-h6 font-weight-bold">Settings</span>
                    </div>

                    <v-list nav class="pa-2">
                        <template v-for="item in visibleUserNavItems" :key="item.tab">
                            <v-list-item @click="activeTab = item.tab" :active="activeTab === item.tab" color="primary"
                                rounded="lg" class="mb-1" :data-testid="item.desktopTestId">
                                <template v-slot:prepend>
                                    <div class="nav-icon-wrapper" :class="{ active: activeTab === item.tab }">
                                        <v-icon :icon="item.icon" size="small"></v-icon>
                                    </div>
                                </template>
                                <v-list-item-title class="text-body-1 font-weight-medium">{{ item.desktopLabel
                                    }}</v-list-item-title>
                                <template v-slot:append>
                                    <v-icon icon="mdi-chevron-right" size="small" class="nav-chevron"></v-icon>
                                </template>
                            </v-list-item>
                        </template>

                        <template v-if="store.loggedInUserIsAdmin">
                            <v-list-subheader class="text-overline mt-2">Administration</v-list-subheader>

                            <template v-for="item in visibleAdminNavItems" :key="item.tab">
                                <v-list-item @click="activeTab = item.tab" :active="activeTab === item.tab"
                                    color="primary" rounded="lg" :class="{ 'mb-1': !isLastAdminItem(item) }"
                                    :data-testid="item.desktopTestId">
                                    <template v-slot:prepend>
                                        <div class="nav-icon-wrapper" :class="{ active: activeTab === item.tab }">
                                            <v-icon :icon="item.icon" size="small"></v-icon>
                                        </div>
                                    </template>
                                    <v-list-item-title class="text-body-1 font-weight-medium">{{ item.desktopLabel
                                        }}</v-list-item-title>
                                    <template v-slot:append>
                                        <v-icon icon="mdi-chevron-right" size="small" class="nav-chevron"></v-icon>
                                    </template>
                                </v-list-item>
                            </template>
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
                    <toolchain-mgmt v-if="activeTab === 'toolchains'" />
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
import ToolchainMgmt from "../components/ToolchainMgmt.vue";
import { useStore } from "../store/store";
import { ref, computed, onBeforeMount } from "vue";
import { useRoute } from "vue-router";
import { settingsService } from "../services";
import { isSuccess } from "../services/api";
import type { Settings } from "../types/settings";
import { emptySettings } from "../types/settings";

type SettingsTab = 'password' | 'tokens' | 'users' | 'groups' | 'config' | 'toolchains'

interface NavItem {
    tab: SettingsTab
    icon: string
    desktopLabel: string
    mobileLabel: string
    adminOnly: boolean
    condition?: () => boolean
    desktopTestId?: string
    mobileTestId?: string
}

const navItems: NavItem[] = [
    {
        tab: 'password',
        icon: 'mdi-key',
        desktopLabel: 'Change Password',
        mobileLabel: 'Change Password',
        adminOnly: false
    },
    {
        tab: 'tokens',
        icon: 'mdi-shield-key',
        desktopLabel: 'Auth. Tokens',
        mobileLabel: 'Auth Tokens',
        adminOnly: false
    },
    {
        tab: 'users',
        icon: 'mdi-account-multiple',
        desktopLabel: 'User Management',
        mobileLabel: 'Users',
        adminOnly: true,
        desktopTestId: 'nav-user-management'
    },
    {
        tab: 'groups',
        icon: 'mdi-account-group',
        desktopLabel: 'Group Management',
        mobileLabel: 'Groups',
        adminOnly: true
    },
    {
        tab: 'config',
        icon: 'mdi-tune',
        desktopLabel: 'Startup Config',
        mobileLabel: 'Config',
        adminOnly: true,
        desktopTestId: 'nav-startup-config',
        mobileTestId: 'nav-startup-config-mobile'
    },
    {
        tab: 'toolchains',
        icon: 'mdi-hammer-wrench',
        desktopLabel: 'Toolchains',
        mobileLabel: 'Toolchains',
        adminOnly: true,
        condition: () => settings.value.toolchain.enabled,
        desktopTestId: 'nav-toolchains',
        mobileTestId: 'nav-toolchains-mobile'
    }
]

const route = useRoute()

const settings = ref<Settings>(emptySettings)
const mobileNavOpen = ref(false)

function getInitialTab(): SettingsTab {
    const tab = route.query.tab as string | undefined
    const validTabs: SettingsTab[] = ['password', 'tokens', 'users', 'groups', 'config', 'toolchains']
    return validTabs.includes(tab as SettingsTab) ? (tab as SettingsTab) : 'password'
}

const activeTab = ref<SettingsTab>(getInitialTab())
const store = useStore()

const visibleUserNavItems = computed(() =>
    navItems.filter(item => !item.adminOnly && (item.condition === undefined || item.condition()))
)

const visibleAdminNavItems = computed(() =>
    navItems.filter(item => item.adminOnly && (item.condition === undefined || item.condition()))
)

function isLastAdminItem(item: NavItem): boolean {
    const items = visibleAdminNavItems.value
    return items.indexOf(item) === items.length - 1
}

function selectTab(tab: SettingsTab) {
    activeTab.value = tab
    mobileNavOpen.value = false
}

function getTabLabel(tab: SettingsTab): string {
    const labels: Record<SettingsTab, string> = {
        'password': 'Password',
        'tokens': 'Tokens',
        'users': 'Users',
        'groups': 'Groups',
        'config': 'Config',
        'toolchains': 'Toolchains'
    }
    return labels[tab]
}

onBeforeMount(async () => {
    const result = await settingsService.getSettings()
    if (isSuccess(result)) {
        settings.value = result.data
    }
})
</script>

<style scoped>
.settings-container {
    padding-bottom: 60px;
    /* Space for footer */
}

/* Mobile Navigation Header */
.mobile-nav-header .sidebar-header {
    display: flex;
    align-items: center;
    padding: 12px 16px;
    background: rgba(var(--v-theme-primary), 0.05);
    border-bottom: 1px solid rgb(var(--v-theme-outline));
}

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

/* Mobile styles */
@media (max-width: 960px) {
    .content-card {
        min-height: 300px;
    }
}
</style>
