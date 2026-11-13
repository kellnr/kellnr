<template>
    <v-container fluid class="pa-0 settings-container">
        <PageHeader title="Settings" />

        <!-- Mobile Navigation Header -->
        <div class="mobile-nav-header d-md-none mb-4">
            <v-card class="sidebar-card" elevation="0">
                <button type="button" class="sidebar-header mobile-nav-toggle" :aria-expanded="mobileNavOpen"
                    aria-controls="settings-nav-mobile" data-testid="settings-nav-toggle"
                    @click="mobileNavOpen = !mobileNavOpen">
                    <v-icon icon="mdi-menu" size="small" class="me-3 sidebar-icon" aria-hidden="true"></v-icon>
                    <span class="sidebar-title">Sections</span>
                    <v-spacer></v-spacer>
                    <v-chip size="small" variant="tonal" color="primary" class="me-2">
                        {{ getTabLabel(activeTab) }}
                    </v-chip>
                    <v-icon :icon="mobileNavOpen ? 'mdi-chevron-up' : 'mdi-chevron-down'" size="small"
                        aria-hidden="true"></v-icon>
                </button>

                <v-expand-transition>
                    <nav v-show="mobileNavOpen" id="settings-nav-mobile" class="settings-nav"
                        aria-label="Settings sections">
                        <template v-for="group in navGroups" :key="group.id">
                            <div v-if="group.label" class="nav-group-label">{{ group.label }}</div>
                            <ul class="nav-list">
                                <li v-for="item in group.items" :key="item.tab">
                                    <a :href="tabHref(item.tab)" class="nav-item"
                                        :class="{ 'nav-item--active': activeTab === item.tab }"
                                        :aria-current="activeTab === item.tab ? 'page' : undefined"
                                        :data-testid="item.mobileTestId" @click="onNavClick($event, item.tab)">
                                        <span class="nav-icon-wrapper">
                                            <v-icon :icon="item.icon" size="small" aria-hidden="true"></v-icon>
                                        </span>
                                        <span class="nav-label">{{ item.label }}</span>
                                    </a>
                                </li>
                            </ul>
                        </template>
                    </nav>
                </v-expand-transition>
            </v-card>
        </div>

        <v-row>
            <!-- Desktop Left sidebar -->
            <v-col cols="12" md="4" xl="3" class="d-none d-md-block">
                <v-card class="sidebar-card" elevation="0">
                    <nav class="settings-nav" aria-label="Settings sections">
                        <template v-for="group in navGroups" :key="group.id">
                            <div v-if="group.label" class="nav-group-label">{{ group.label }}</div>
                            <ul class="nav-list">
                                <li v-for="item in group.items" :key="item.tab">
                                    <a :href="tabHref(item.tab)" class="nav-item"
                                        :class="{ 'nav-item--active': activeTab === item.tab }"
                                        :aria-current="activeTab === item.tab ? 'page' : undefined"
                                        :data-testid="item.desktopTestId" @click="onNavClick($event, item.tab)">
                                        <span class="nav-icon-wrapper">
                                            <v-icon :icon="item.icon" size="small" aria-hidden="true"></v-icon>
                                        </span>
                                        <span class="nav-label">{{ item.label }}</span>
                                        <v-icon icon="mdi-chevron-right" size="small" class="nav-chevron"
                                            aria-hidden="true"></v-icon>
                                    </a>
                                </li>
                            </ul>
                        </template>
                    </nav>
                </v-card>

                <!-- Admins decide what their team spends on tools, so ask them -->
                <div v-if="store.loggedInUserIsAdmin && sponsorLinksEnabled" class="sponsor-note"
                    data-testid="settings-sponsor-note">
                    <v-icon icon="mdi-heart" size="small" class="sponsor-icon"></v-icon>
                    <span>
                        Kellnr is maintained by one person in their spare time. If it saves your team time,
                        consider <a href="https://kellnr.io/sponsor" target="_blank" rel="noopener noreferrer">sponsoring it</a>.
                    </span>
                </div>
            </v-col>

            <!-- Content area -->
            <v-col cols="12" md="8" xl="9">
                <v-card class="content-card" elevation="0">
                    <change-password v-if="activeTab === 'password'" />
                    <auth-token v-if="activeTab === 'tokens'" />
                    <user-mgmt v-if="activeTab === 'users'" :sso-enforced="ssoEnforced" />
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
import { ref, computed, onBeforeMount, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { settingsService } from "../services";
import { isSuccess } from "../services/api";
import type { Settings } from "../types/settings";
import { emptySettings } from "../types/settings";
import { SETTINGS_SECTION_TITLES, type SettingsTab } from "../common/settingsSections";
import { PageHeader } from "../components/common";

interface NavItem {
    tab: SettingsTab
    icon: string
    label: string
    adminOnly: boolean
    condition?: () => boolean
    desktopTestId?: string
    mobileTestId?: string
}

const navItems: NavItem[] = [
    {
        tab: 'password',
        icon: 'mdi-key',
        label: SETTINGS_SECTION_TITLES.password,
        adminOnly: false,
        desktopTestId: 'nav-change-password',
        condition: () => !ssoEnforced.value
    },
    {
        tab: 'tokens',
        icon: 'mdi-shield-key',
        label: SETTINGS_SECTION_TITLES.tokens,
        adminOnly: false,
        desktopTestId: 'nav-auth-tokens'
    },
    {
        tab: 'users',
        icon: 'mdi-account-multiple',
        label: SETTINGS_SECTION_TITLES.users,
        adminOnly: true,
        desktopTestId: 'nav-user-management'
    },
    {
        tab: 'groups',
        icon: 'mdi-account-group',
        label: SETTINGS_SECTION_TITLES.groups,
        adminOnly: true,
        desktopTestId: 'nav-groups'
    },
    {
        tab: 'config',
        icon: 'mdi-tune',
        label: SETTINGS_SECTION_TITLES.config,
        adminOnly: true,
        desktopTestId: 'nav-startup-config',
        mobileTestId: 'nav-startup-config-mobile'
    },
    {
        tab: 'toolchains',
        icon: 'mdi-hammer-wrench',
        label: SETTINGS_SECTION_TITLES.toolchains,
        adminOnly: true,
        condition: () => settings.value.toolchain.enabled,
        desktopTestId: 'nav-toolchains',
        mobileTestId: 'nav-toolchains-mobile'
    }
]

const route = useRoute()
const router = useRouter()

const settings = ref<Settings>(emptySettings)
const ssoEnforced = ref(false)
const sponsorLinksEnabled = ref(false)
const mobileNavOpen = ref(false)

function getTabFromRoute(): SettingsTab {
    const tab = route.query.tab as string | undefined
    const validTabs: SettingsTab[] = ['password', 'tokens', 'users', 'groups', 'config', 'toolchains']
    return validTabs.includes(tab as SettingsTab) ? (tab as SettingsTab) : 'password'
}

const activeTab = ref<SettingsTab>(getTabFromRoute())

// Keep the section in the URL, so it survives a reload and can be shared
watch(activeTab, (tab) => {
    if (route.query.tab !== tab) {
        router.replace({ query: { ...route.query, tab } })
    }
})

// Back and forward in the browser switch between sections
watch(() => route.query.tab, () => {
    if (route.query.tab !== undefined) {
        activeTab.value = getTabFromRoute()
    }
})
const store = useStore()

const visibleUserNavItems = computed(() =>
    navItems.filter(item => !item.adminOnly && (item.condition === undefined || item.condition()))
)

const visibleAdminNavItems = computed(() =>
    navItems.filter(item => item.adminOnly && (item.condition === undefined || item.condition()))
)

interface NavGroup {
    id: string
    label?: string
    items: NavItem[]
}

const navGroups = computed<NavGroup[]>(() => {
    const groups: NavGroup[] = [{ id: 'user', items: visibleUserNavItems.value }]
    if (store.loggedInUserIsAdmin) {
        groups.push({ id: 'admin', label: 'Administration', items: visibleAdminNavItems.value })
    }
    return groups
})

function tabHref(tab: SettingsTab): string {
    return router.resolve({ query: { ...route.query, tab } }).href
}

// The nav items are real links, so opening a section in a new tab keeps working
function onNavClick(event: MouseEvent, tab: SettingsTab) {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
        return
    }
    event.preventDefault()
    activeTab.value = tab
    mobileNavOpen.value = false
}

function getTabLabel(tab: SettingsTab): string {
    return SETTINGS_SECTION_TITLES[tab]
}

onBeforeMount(async () => {
    const result = await settingsService.getSettings()
    if (isSuccess(result)) {
        settings.value = result.data
    }

    const oauth2Result = await settingsService.getOAuth2Config()
    if (isSuccess(oauth2Result)) {
        ssoEnforced.value = oauth2Result.data.enforced
        // The password tab is hidden when SSO is enforced, so move off it.
        // Any other tab, including a deep-linked one, stays as selected.
        if (ssoEnforced.value && activeTab.value === 'password') {
            activeTab.value = 'tokens'
        }
    }

    const sponsorResult = await settingsService.getSponsorLinksEnabled()
    if (isSuccess(sponsorResult)) {
        sponsorLinksEnabled.value = sponsorResult.data.enabled
    }
})
</script>

<style scoped>
/* Mobile Navigation Header */
.mobile-nav-toggle {
    width: 100%;
    padding: 12px 16px;
    color: inherit;
    font: inherit;
    text-align: left;
    background: none;
    border: none;
    cursor: pointer;
}

.mobile-nav-toggle:focus-visible {
    outline: 2px solid rgb(var(--v-theme-primary));
    outline-offset: -2px;
}

/* Sidebar Card */
.sidebar-card {
    background: rgb(var(--v-theme-surface));
    border: 1px solid rgb(var(--v-theme-outline));
    border-radius: var(--kellnr-radius-card);
    overflow: hidden;
}

.sidebar-header {
    display: flex;
    align-items: center;
    border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.sidebar-icon {
    color: rgb(var(--v-theme-on-surface-variant));
}

.sidebar-title {
    font-size: 1.0625rem;
    font-weight: 600;
}

/* Navigation */
.settings-nav {
    padding: 8px;
}

.nav-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
}

.nav-group-label {
    margin: 12px 8px 4px;
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: rgb(var(--v-theme-on-surface-variant));
}

.nav-item {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 44px;
    padding: 6px 8px;
    border-radius: var(--kellnr-radius);
    color: rgb(var(--v-theme-on-surface));
    font-size: 0.875rem;
    font-weight: 500;
    text-decoration: none;
    transition: background-color 0.2s ease;
}

.nav-item:hover {
    background: rgba(var(--v-theme-on-surface), 0.06);
}

/* Only keyboard focus gets a ring, so a clicked item does not stay highlighted */
.nav-item:focus-visible {
    outline: 2px solid rgb(var(--v-theme-primary));
    outline-offset: -2px;
}

.nav-item--active,
.nav-item--active:hover {
    background: rgba(var(--v-theme-primary), 0.12);
    font-weight: 600;
}

.nav-label {
    flex: 1;
    min-width: 0;
}

.nav-icon-wrapper {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: var(--kellnr-radius);
    background: rgba(var(--v-theme-primary), 0.08);
    color: rgb(var(--v-theme-primary));
    transition: background-color 0.2s ease, color 0.2s ease;
}

.nav-item--active .nav-icon-wrapper {
    background: rgb(var(--v-theme-primary));
    color: rgb(var(--v-theme-on-primary));
}

.nav-chevron {
    opacity: 0.3;
    transition: opacity 0.2s ease;
}

.nav-item:hover .nav-chevron {
    opacity: 0.7;
}

.nav-item--active .nav-chevron {
    opacity: 1;
}

/* Sponsor Note */
.sponsor-note {
    display: flex;
    gap: 10px;
    margin-top: 16px;
    padding: 12px 16px;
    font-size: 0.8125rem;
    line-height: 1.5;
    color: rgb(var(--v-theme-on-surface-variant));
    border: 1px solid rgb(var(--v-theme-outline));
    border-radius: var(--kellnr-radius-card);
}

.sponsor-icon {
    margin-top: 2px;
    color: rgb(var(--v-theme-error));
}

.sponsor-note a {
    font-weight: 600;
    color: rgb(var(--v-theme-primary));
}

/* Content Card */
.content-card {
    background: rgb(var(--v-theme-surface));
    border: 1px solid rgb(var(--v-theme-outline));
    border-radius: var(--kellnr-radius-card);
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
