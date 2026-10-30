<template>
  <v-container v-if="crate != null" fluid class="pa-0">
    <!-- Crate header with the tabs below it, directly on the page -->
    <header class="crate-header mb-6">
      <div class="d-flex flex-wrap align-baseline">
        <h1 class="text-headline-large font-weight-bold my-0 me-3 text-break crate-title" data-testid="crate-title">{{ crate.name }}</h1>
        <span class="text-title-large tabular-nums version-text" data-testid="crate-version">{{ selected_version.version }}</span>
        <!-- Owners and admins see who can download a restricted crate -->
        <CrateAccessBadge v-if="canManageCrate" :crate-name="crate.name" :refresh-key="accessVersion"
          class="ms-3 align-self-center" @open="openAccessSettings" />
      </div>
      <p v-if="crate.description != null" class="description-text mt-2 mb-0" data-testid="crate-description">
        {{ crate.description }}
      </p>

      <!-- Categories and keywords link to the crates that share them -->
      <div v-if="crate.categories.length > 0 || crate.keywords.length > 0" class="crate-tags mt-3"
        data-testid="crate-tags">
        <router-link v-for="category in crate.categories" :key="`category-${category}`"
          :to="{ name: 'Crates', query: { category } }" class="tag-chip" :title="`Crates in ${category}`"
          data-testid="crate-category">
          <v-icon icon="mdi-folder-outline" size="x-small" />
          <span>{{ category }}</span>
        </router-link>
        <router-link v-for="keyword in crate.keywords" :key="`keyword-${keyword}`"
          :to="{ name: 'Crates', query: { keyword } }" class="tag-chip" :title="`Crates with the keyword ${keyword}`"
          data-testid="crate-keyword">#{{ keyword }}</router-link>
      </div>

      <!-- Arrows show that more tabs are hidden on narrow screens -->
      <v-tabs :model-value="tab" color="primary" slider-color="primary" show-arrows density="compact"
        class="crate-tabs mt-4" @update:model-value="selectTab">
        <v-tab v-if="selected_version.readme" value="readme" data-testid="crate-tab-readme">Readme</v-tab>
        <v-tab value="about" data-testid="crate-tab-about">About</v-tab>
        <v-tab value="dependencies" data-testid="crate-tab-deps">Dependencies</v-tab>
        <v-tab value="versions" data-testid="crate-tab-versions">Versions</v-tab>
        <v-tab value="dependents" data-testid="crate-tab-dependents">Used by</v-tab>
        <v-tab v-if="canManageCrate" value="settings" data-testid="crate-tab-settings">Settings</v-tab>
        <v-tab v-if="store.loggedInUserIsAdmin" value="admin" data-testid="crate-tab-admin">Admin</v-tab>
      </v-tabs>
    </header>

    <!-- Snackbar for copy notification -->
    <v-snackbar v-model="showSnackbar" :timeout="3000" color="success" location="bottom" data-testid="snackbar">
      {{ snackbarText }}
      <template v-slot:actions>
        <v-btn variant="text" icon="mdi-close" @click="showSnackbar = false"></v-btn>
      </template>
    </v-snackbar>

    <v-row>
      <!-- Main Content Area -->
      <v-col cols="12" md="8">
        <!-- Readme Tab -->
        <v-card v-if="tab === 'readme'" class="mb-4 content-card" elevation="0">
          <Readme :readme="selected_version.readme"></Readme>
        </v-card>

        <!-- Versions Tab -->
        <v-card v-if="tab === 'versions'" class="mb-4 content-card" elevation="0" data-testid="crate-versions">
          <v-card-text>
            <!-- Versions are grouped by semver compatibility, like 1.x or 0.3.x -->
            <section v-for="group in versionGroups" :key="group.label" class="version-group"
              data-testid="version-group">
              <h3 class="version-group-label" data-testid="version-group-label">
                {{ group.label }}
                <span class="version-group-count">{{ group.versions.length }}</span>
              </h3>
              <Version v-for="version in group.versions" :key="version.version" :name="crate.name" :version="version" />
            </section>
          </v-card-text>
        </v-card>

        <!-- Dependencies Tab -->
        <template v-if="tab === 'dependencies'">
          <!-- Normal Dependencies -->
          <v-card v-if="sortedDependencies.normal.length > 0" class="mb-4 content-card" elevation="0"
            data-testid="crate-deps-normal">
            <SectionHeader icon="mdi-package-variant" title="Dependencies" :count="sortedDependencies.normal.length" />
            <v-card-text>
              <Dependency v-for="dep in sortedDependencies.normal" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" :desc="dep.description" :latest-version="dep.latest_version"
                :outdated="dep.outdated" />
            </v-card-text>
          </v-card>

          <!-- Dev Dependencies -->
          <v-card v-if="sortedDependencies.dev.length > 0" class="mb-4 content-card" elevation="0"
            data-testid="crate-deps-dev">
            <SectionHeader icon="mdi-flask-outline" title="Development dependencies" :count="sortedDependencies.dev.length" />
            <v-card-text>
              <Dependency v-for="dep in sortedDependencies.dev" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" :desc="dep.description" :latest-version="dep.latest_version"
                :outdated="dep.outdated" />
            </v-card-text>
          </v-card>

          <!-- Build dependencies -->
          <v-card v-if="sortedDependencies.build.length > 0" class="mb-4 content-card" elevation="0"
            data-testid="crate-deps-build">
            <SectionHeader icon="mdi-hammer-wrench" title="Build dependencies" :count="sortedDependencies.build.length" />
            <v-card-text>
              <Dependency v-for="dep in sortedDependencies.build" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" :desc="dep.description" :latest-version="dep.latest_version"
                :outdated="dep.outdated" />
            </v-card-text>
          </v-card>
        </template>

        <!-- Used by Tab, loads the dependents when it is opened -->
        <CrateDependentsTab v-if="tab === 'dependents'" :crate-name="crate.name" />

        <!-- About (Meta) Tab -->
        <v-card v-if="tab === 'about'" class="mb-4 content-card" elevation="0">
          <About :crate="crate" :selected-version="selected_version" :flattened-features="flattenedFeatures"
            :sorted-owners="sortedOwners" />
        </v-card>

        <!-- Settings Tab -->
        <CrateSettingsTab
          v-if="tab === 'settings'"
          :crate-name="crate.name"
          @owners-changed="handleOwnersChanged"
          @access-changed="accessVersion++"
        />

        <!-- Admin Tab -->
        <CrateAdminTab
          v-if="tab === 'admin'"
          :crate-name="crate.name"
          :version="selected_version.version"
        />
      </v-col>

      <!-- Sidebar Column -->
      <v-col cols="12" md="4">
        <CrateSidebar :crate-name="crate.name" :version="selected_version.version" :published="selected_version.created"
          :publisher="selected_version.publisher ?? ''" :publisher-token="selected_version.publisher_token ?? ''"
          :version-downloads="selected_version.downloads" :total-downloads="crate.total_downloads"
          :documentation-link="docLink" :can-build-docs="showBuildRustdoc()" :repository="crate.repository ?? ''"
          :homepage="crate.homepage ?? ''" :license="selected_version.license ?? ''"
          @copy-to-clipboard="copyToClipboard" @open-docs="openDocsPage"
          @build-docs="buildDoc(crate.name, selected_version.version)" />
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
import Dependency from "../components/Dependency.vue";
import Version from "../components/Version.vue";
import Readme from "../components/Readme.vue";
import CrateSidebar from "../components/CrateSidebar.vue";
import About from "../components/About.vue";
import CrateSettingsTab from "../components/crate/CrateSettingsTab.vue";
import CrateAdminTab from "../components/crate/CrateAdminTab.vue";
import CrateDependentsTab from "../components/crate/CrateDependentsTab.vue";
import CrateAccessBadge from "../components/crate/CrateAccessBadge.vue";
import { SectionHeader } from "../components/common";
import { computed, nextTick, onBeforeMount, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { defaultCrateData, defaultCrateVersionData } from "../types/crate_data";
import { safeUrl } from "../common/url";
import { versionGroup } from "../common/format";
import type { CrateData, CrateVersionData, CrateRegistryDep } from "../types/crate_data";
import { crateService, settingsService } from "../services";
import { isSuccess } from "../services/api";
import { useStore } from "../store/store";

const crateData = ref<CrateData>(defaultCrateData);
const router = useRouter()
const route = useRoute()
const selected_version = ref<CrateVersionData>(defaultCrateVersionData)
const tab = ref<CrateTab>("about");
const store = useStore();
const docsEnabled = ref(false);
// Counts the changes in the settings tab, so the access badge reloads
const accessVersion = ref(0);

// Snackbar refs
const showSnackbar = ref(false);
const snackbarText = ref('');

// Expose crate as computed for template compatibility
const crate = computed(() => crateData.value);

type CrateTab = "readme" | "about" | "dependencies" | "versions" | "dependents" | "settings" | "admin"

// The tab from the URL if the crate has it, else the readme if there is one
function tabFromRoute(): CrateTab {
  const requested = route.query.tab?.toString()
  const available: CrateTab[] = ["about", "dependencies", "versions", "dependents"]
  if (selected_version.value.readme) available.push("readme")
  if (canManageCrate.value) available.push("settings")
  if (store.loggedInUserIsAdmin) available.push("admin")
  if (available.includes(requested as CrateTab)) {
    return requested as CrateTab
  }
  return selected_version.value.readme ? "readme" : "about"
}

// Keep the tab in the URL, so it survives a reload and can be shared
function selectTab(value: unknown) {
  tab.value = value as CrateTab
  router.replace({ query: { ...route.query, tab: tab.value } })
}

// The access badge opens the settings tab at the access lists
async function openAccessSettings() {
  selectTab("settings")
  await nextTick()
  document.getElementById("crate-access")?.scrollIntoView({ behavior: "smooth", block: "start" })
}

// Crate metadata is user-provided, only link to http(s) or relative URLs
const docLink = computed(() => {
  return safeUrl(selected_version.value.documentation);
})

// The versions are sorted newest first, so are the groups
const versionGroups = computed(() => {
  const groups: { label: string, versions: CrateVersionData[] }[] = [];
  for (const version of crateData.value.versions) {
    const label = versionGroup(version.version);
    const group = groups.find(g => g.label === label);
    if (group) {
      group.versions.push(version);
    } else {
      groups.push({ label, versions: [version] });
    }
  }
  return groups;
});

function getDependenciesByKind(kind: string): CrateRegistryDep[] {
  return sortByName(
    selected_version.value.dependencies.filter(dep => dep.kind === kind)
  );
}

const sortedDependencies = computed(() => ({
  normal: getDependenciesByKind('normal'),
  dev: getDependenciesByKind('dev'),
  build: getDependenciesByKind('build'),
}));

const flattenedFeatures = computed(() => {
  const features = selected_version.value.features;
  const flattened: string[] = [];
  for (let key in features) {
    if (features[key].length > 0) {
      flattened.push(key + ": " + features[key].join(", "));
    } else {
      flattened.push(key);
    }
  }
  flattened.sort();
  return flattened;
});

const sortedOwners = computed(() => {
  const users = crateData.value.owners ?? [];
  return [...users].sort();
});

// Handle owners changed event from settings tab
function handleOwnersChanged(owners: string[]) {
  crateData.value.owners = owners;
}

// Admins and crate owners can manage owners, crate users and crate groups
const canManageCrate = computed(() => {
  if (store.loggedInUserIsAdmin) {
    return true;
  }
  return store.loggedInUser !== null && crateData.value.owners.includes(store.loggedInUser);
});

function showBuildRustdoc(): boolean {
  if (!docsEnabled.value) {
    return false;
  }
  // Show the option to build the docs, if the current logged-in user is admin or owns the crate
  return canManageCrate.value;
}

async function buildDoc(crateName: string, version: string) {
  const result = await crateService.buildDocs(crateName, version)
  if (isSuccess(result)) {
    router.push({ name: "DocQueue" })
  } else {
    console.error('Failed to build docs:', result.error)
  }
}

function sortByName(deps: Array<CrateRegistryDep>) {
  return deps.sort((a, b) => {
    if (a.name < b.name) {
      return -1;
    }
    if (a.name > b.name) {
      return 1;
    }
    return 0;
  });
}

function copyToClipboard(text: string) {
  navigator.clipboard.writeText(text)
    .then(() => {
      snackbarText.value = "Copied to clipboard!";
      showSnackbar.value = true;
    })
    .catch(() => {
      snackbarText.value = "Failed to copy. Please try again.";
      showSnackbar.value = true;
    });
}

function openDocsPage() {
  if (docLink.value) {
    // Never give the docs page a handle to the UI window.
    window.open(docLink.value, "_blank", "noopener,noreferrer");
  } else {
    router.push({ name: "PublishDocs" })
  }
}

async function getCrateData(name: string, version?: string) {
  const result = await crateService.getCrateData(name)
  if (isSuccess(result)) {
    crateData.value = result.data;
    version = version ?? crateData.value.max_version;
    selected_version.value = crateData.value.versions.find((cvd: CrateVersionData) => {
      return cvd.version == version;
    }) ?? defaultCrateVersionData;

    tab.value = tabFromRoute();
  } else {
    console.error('Failed to load crate data:', result.error)
  }
}

async function getAllData() {
  const version = route.query.version?.toString();
  const name = route.query.name?.toString() ?? "";

  if (name !== "") {
    await getCrateData(name, version);
  }

  const docsResult = await settingsService.getDocsEnabled();
  if (isSuccess(docsResult)) {
    docsEnabled.value = docsResult.data.enabled;
  }
}

onBeforeMount(() => {
  getAllData()
})

// Reloads the data when another crate or version is opened. A tab change
// only updates the "tab" query parameter and needs no reload.
watch(() => [route.query.name, route.query.version], () => {
  getAllData()
})

// Back and forward in the browser switch between tabs of the same crate. The
// login state is restored after a reload, which makes the owner and admin
// tabs available, so a link to one of them is checked again then.
watch(() => [route.query.tab, canManageCrate.value, store.loggedInUserIsAdmin], () => {
  tab.value = tabFromRoute()
})

</script>

<style scoped>
.text-break {
  word-break: break-word;
}

/* Header */
.crate-title {
  color: rgb(var(--v-theme-on-background));
  line-height: 1.2;
}

.version-text {
  color: rgb(var(--v-theme-on-surface-variant));
}

.description-text {
  color: rgb(var(--v-theme-on-surface-variant));
  font-size: 1rem;
  line-height: 1.6;
  max-width: 70ch;
}

.crate-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.tag-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 10px;
  border-radius: 9999px;
  font-size: 13px;
  line-height: 1.6;
  text-decoration: none;
  color: rgb(var(--v-theme-on-surface-variant));
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  transition: border-color 0.2s ease, color 0.2s ease;
}

.tag-chip:hover,
.tag-chip:focus-visible {
  color: rgb(var(--v-theme-primary));
  border-color: rgb(var(--v-theme-primary));
}

/* Version groups */
.version-group + .version-group {
  margin-top: 20px;
}

.version-group-label {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
  font-family: var(--kellnr-font-mono);
  color: rgb(var(--v-theme-on-surface));
}

.version-group-count {
  font-family: var(--kellnr-font-sans);
  font-size: 11px;
  font-weight: 500;
  padding: 1px 8px;
  border-radius: 9999px;
  background: rgb(var(--v-theme-surface-variant));
  color: rgb(var(--v-theme-on-surface-variant));
}

/* The tabs sit on a line that spans the page */
.crate-tabs {
  background: transparent;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.crate-tabs :deep(.v-tab) {
  text-transform: none;
  font-weight: 500;
  letter-spacing: normal;
  min-width: 0;
}

/* Content Cards */
.content-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--kellnr-radius-card);
  overflow: hidden;
}

:deep(.v-card-title) {
  color: rgb(var(--v-theme-on-surface));
}

:deep(.v-card-text) {
  color: rgb(var(--v-theme-on-surface));
}

/* List items */
:deep(.v-list) {
  background: transparent;
}

:deep(.v-list-item) {
  border-radius: var(--kellnr-radius);
}

:deep(.v-list-item:hover) {
  background: rgb(var(--v-theme-surface-variant));
}

/* Alerts */
:deep(.v-alert) {
  border-radius: var(--kellnr-radius);
}

/* Form fields */
:deep(.v-text-field) {
  margin-bottom: 8px;
}
</style>
