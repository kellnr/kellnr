<template>
  <v-container v-if="crate != null" fluid class="pa-0">
    <!-- Crate header with the tabs below it, directly on the page -->
    <header class="crate-header mb-6">
      <div class="d-flex flex-wrap align-baseline">
        <h1 class="text-headline-large font-weight-bold my-0 me-3 text-break crate-title" data-testid="crate-title">{{ crate.name }}</h1>
        <span class="text-title-large tabular-nums version-text" data-testid="crate-version">{{ selected_version.version }}</span>
      </div>
      <p v-if="crate.description != null" class="description-text mt-2 mb-0" data-testid="crate-description">
        {{ crate.description }}
      </p>

      <!-- Arrows show that more tabs are hidden on narrow screens -->
      <v-tabs :model-value="tab" color="primary" slider-color="primary" show-arrows density="compact"
        class="crate-tabs mt-4" @update:model-value="selectTab">
        <v-tab v-if="selected_version.readme" value="readme" data-testid="crate-tab-readme">Readme</v-tab>
        <v-tab value="about" data-testid="crate-tab-about">About</v-tab>
        <v-tab value="dependencies" data-testid="crate-tab-deps">Dependencies</v-tab>
        <v-tab value="versions" data-testid="crate-tab-versions">Versions</v-tab>
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
            <Version v-for="version in crate.versions" :key="version.version" :name="crate.name"
              :version="version.version" :last_updated="version.created" :downloads="version.downloads.toString()" />
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
                :registry="dep.registry" />
            </v-card-text>
          </v-card>

          <!-- Dev Dependencies -->
          <v-card v-if="sortedDependencies.dev.length > 0" class="mb-4 content-card" elevation="0"
            data-testid="crate-deps-dev">
            <SectionHeader icon="mdi-flask-outline" title="Development dependencies" :count="sortedDependencies.dev.length" />
            <v-card-text>
              <Dependency v-for="dep in sortedDependencies.dev" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" />
            </v-card-text>
          </v-card>

          <!-- Build dependencies -->
          <v-card v-if="sortedDependencies.build.length > 0" class="mb-4 content-card" elevation="0"
            data-testid="crate-deps-build">
            <SectionHeader icon="mdi-hammer-wrench" title="Build dependencies" :count="sortedDependencies.build.length" />
            <v-card-text>
              <Dependency v-for="dep in sortedDependencies.build" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" :desc="dep.description" />
            </v-card-text>
          </v-card>
        </template>

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
        <CrateSidebar :crate-name="crate.name" :version="selected_version.version" :last-updated="crate.last_updated"
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
import { SectionHeader } from "../components/common";
import { computed, onBeforeMount, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { defaultCrateData, defaultCrateVersionData } from "../types/crate_data";
import { safeUrl } from "../common/url";
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

// Snackbar refs
const showSnackbar = ref(false);
const snackbarText = ref('');

// Expose crate as computed for template compatibility
const crate = computed(() => crateData.value);

type CrateTab = "readme" | "about" | "dependencies" | "versions" | "settings" | "admin"

// The tab from the URL if the crate has it, else the readme if there is one
function tabFromRoute(): CrateTab {
  const requested = route.query.tab?.toString()
  const available: CrateTab[] = ["about", "dependencies", "versions"]
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

// Crate metadata is user-provided, only link to http(s) or relative URLs
const docLink = computed(() => {
  return safeUrl(selected_version.value.documentation);
})

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
