<template>
  <v-container fluid class="pa-0">
    <div v-if="pageState.kind === 'loading'" class="d-flex justify-center py-12" data-testid="crate-loading">
      <v-progress-circular indeterminate color="primary" aria-label="Loading crate" />
    </div>

    <!-- Unknown crate or version -->
    <div v-else-if="pageState.kind === 'not-found'" class="crate-state" data-testid="crate-not-found">
      <EmptyState icon="mdi-package-variant-remove" :message="notFoundMessage" />
      <div class="d-flex flex-wrap justify-center ga-3 mt-4">
        <v-btn v-if="pageState.versionMissing" color="primary" variant="flat"
          :to="{ name: 'Crate', query: { name: requestedName } }" data-testid="crate-not-found-latest">
          Show the latest version
        </v-btn>
        <v-btn :color="pageState.versionMissing ? undefined : 'primary'"
          :variant="pageState.versionMissing ? 'outlined' : 'flat'" :to="{ name: 'Crates' }"
          data-testid="crate-not-found-back">
          Browse all crates
        </v-btn>
      </div>
    </div>

    <div v-else-if="pageState.kind === 'error'" class="crate-state" data-testid="crate-load-error">
      <EmptyState icon="mdi-alert-circle-outline" :message="`Failed to load ${requestedName}: ${pageState.message}`" />
      <div class="d-flex flex-wrap justify-center ga-3 mt-4">
        <v-btn color="primary" variant="flat" prepend-icon="mdi-refresh" data-testid="crate-load-retry"
          @click="getAllData">
          Try again
        </v-btn>
        <v-btn variant="outlined" :to="{ name: 'Crates' }" data-testid="crate-load-error-back">
          Browse all crates
        </v-btn>
      </div>
    </div>

    <template v-else>
      <!-- Crate header with the tabs below it, directly on the page -->
      <header class="crate-header">
        <div class="d-flex flex-wrap align-center ga-3">
          <h1 class="text-headline-large font-weight-bold my-0 text-break crate-title" data-testid="crate-title">{{
            crate.name }}</h1>
          <span class="text-title-large tabular-nums version-text" data-testid="crate-version">{{
            selected_version.version }}</span>
          <v-select :model-value="selected_version.version" :items="versionItems" aria-label="Select version"
            density="compact" variant="outlined" hide-details class="version-select"
            data-testid="crate-version-select" @update:model-value="selectVersion" />
          <!-- Owners and admins see who can download a restricted crate -->
          <CrateAccessBadge v-if="canManageCrate" :crate-name="crate.name" :refresh-key="accessVersion"
            @open="openAccessSettings" />
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
      </header>

      <!-- Small screens show the install snippets before the tabs, larger ones in the sidebar -->
      <v-card v-if="!mdAndUp" elevation="0" class="install-card mt-4">
        <v-card-text>
          <CrateInstall :crate-name="crate.name" :version="selected_version.version"
            @copy-to-clipboard="copyToClipboard" />
        </v-card-text>
      </v-card>

      <!-- Sticks below the app bar while the page scrolls. Arrows show that
           more tabs are hidden on narrow screens. -->
      <div class="crate-tabs-bar mt-4 mb-6">
        <v-tabs :model-value="tab" color="primary" slider-color="primary" show-arrows density="compact"
          class="crate-tabs" @update:model-value="selectTab">
          <v-tab v-if="selected_version.readme" value="readme" data-testid="crate-tab-readme">Readme</v-tab>
          <v-tab value="about" data-testid="crate-tab-about">About</v-tab>
          <v-tab value="dependencies" data-testid="crate-tab-deps">Dependencies</v-tab>
          <v-tab value="versions" data-testid="crate-tab-versions">Versions</v-tab>
          <v-tab value="dependents" data-testid="crate-tab-dependents">Used by</v-tab>
          <v-tab v-if="canManageCrate" value="settings" data-testid="crate-tab-settings">Settings</v-tab>
          <v-tab v-if="store.loggedInUserIsAdmin" value="admin" data-testid="crate-tab-admin">Admin</v-tab>
        </v-tabs>
      </div>

      <v-row>
        <!-- Main Content Area -->
        <v-col cols="12" md="8">
          <!-- Readme Tab -->
          <v-card v-if="tab === 'readme'" class="mb-4 content-card" elevation="0">
            <Readme :readme="selected_version.readme ?? null"></Readme>
          </v-card>

          <!-- Versions Tab -->
          <CrateVersionsTab v-if="tab === 'versions'" :crate-name="crate.name" :versions="crate.versions"
            :can-manage="canChangeYank" @changed="refreshCrateData" />

          <!-- Dependencies Tab -->
          <template v-if="tab === 'dependencies'">
            <v-card v-if="!hasDependencies" class="mb-4 content-card" elevation="0" data-testid="crate-deps-empty">
              <v-card-text>
                <EmptyState icon="mdi-package-variant" message="No dependencies" />
              </v-card-text>
            </v-card>

            <!-- Normal Dependencies -->
            <v-card v-if="sortedDependencies.normal.length > 0" class="mb-4 content-card" elevation="0"
              data-testid="crate-deps-normal">
              <SectionHeader icon="mdi-package-variant" title="Dependencies" :count="sortedDependencies.normal.length" />
              <v-card-text>
                <Dependency v-for="dep in sortedDependencies.normal" :key="dep.name" :name="dep.name"
                  :version="dep.version_req" :registry="dep.registry" :desc="dep.description"
                  :latest-version="dep.latest_version" :outdated="dep.outdated" />
              </v-card-text>
            </v-card>

            <!-- Dev Dependencies -->
            <v-card v-if="sortedDependencies.dev.length > 0" class="mb-4 content-card" elevation="0"
              data-testid="crate-deps-dev">
              <SectionHeader icon="mdi-flask-outline" title="Development dependencies"
                :count="sortedDependencies.dev.length" />
              <v-card-text>
                <Dependency v-for="dep in sortedDependencies.dev" :key="dep.name" :name="dep.name"
                  :version="dep.version_req" :registry="dep.registry" :desc="dep.description"
                  :latest-version="dep.latest_version" :outdated="dep.outdated" />
              </v-card-text>
            </v-card>

            <!-- Build dependencies -->
            <v-card v-if="sortedDependencies.build.length > 0" class="mb-4 content-card" elevation="0"
              data-testid="crate-deps-build">
              <SectionHeader icon="mdi-hammer-wrench" title="Build dependencies"
                :count="sortedDependencies.build.length" />
              <v-card-text>
                <Dependency v-for="dep in sortedDependencies.build" :key="dep.name" :name="dep.name"
                  :version="dep.version_req" :registry="dep.registry" :desc="dep.description"
                  :latest-version="dep.latest_version" :outdated="dep.outdated" />
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
            :versions="crate.versions"
            @show-versions="selectTab('versions')"
          />
        </v-col>

        <!-- Sidebar Column -->
        <v-col cols="12" md="4">
          <CrateSidebar :crate-name="crate.name" :version="selected_version.version" :published="selected_version.created"
            :publisher="selected_version.publisher ?? ''" :publisher-token="selected_version.publisher_token ?? ''"
            :version-downloads="selected_version.downloads" :total-downloads="crate.total_downloads"
            :documentation-link="docLink" :can-build-docs="showBuildRustdoc()" :repository="crate.repository ?? ''"
            :homepage="crate.homepage ?? ''" :license="selected_version.license ?? ''" :show-install="mdAndUp"
            @copy-to-clipboard="copyToClipboard" @open-docs="openDocsPage"
            @build-docs="buildDoc(crate.name, selected_version.version)" />
        </v-col>
      </v-row>
    </template>
  </v-container>
</template>

<script setup lang="ts">
import Dependency from "../components/Dependency.vue";
import Readme from "../components/Readme.vue";
import CrateSidebar from "../components/CrateSidebar.vue";
import About from "../components/About.vue";
import CrateSettingsTab from "../components/crate/CrateSettingsTab.vue";
import CrateAdminTab from "../components/crate/CrateAdminTab.vue";
import CrateDependentsTab from "../components/crate/CrateDependentsTab.vue";
import CrateAccessBadge from "../components/crate/CrateAccessBadge.vue";
import CrateInstall from "../components/crate/CrateInstall.vue";
import CrateVersionsTab from "../components/crate/CrateVersionsTab.vue";
import { EmptyState, SectionHeader } from "../components/common";
import { computed, nextTick, onBeforeMount, ref, watch } from "vue";
import { useNotification } from "../composables";
import { useRoute, useRouter } from "vue-router";
import { useDisplay } from "vuetify";
import { defaultCrateData, defaultCrateVersionData } from "../types/crate_data";
import { safeUrl } from "../common/url";
import { crateLoadOutcome, findVersion, versionSelectItems } from "../common/crate";
import { setPageTitle } from "../common/pageTitle";
import type { CrateData, CrateVersionData, CrateRegistryDep } from "../types/crate_data";
import { crateService, settingsService } from "../services";
import { isSuccess } from "../services/api";
import { useStore } from "../store/store";

type PageState =
  | { kind: "loading" }
  | { kind: "found" }
  | { kind: "not-found"; versionMissing: boolean }
  | { kind: "error"; message: string };

const crateData = ref<CrateData>(defaultCrateData);
const router = useRouter()
const route = useRoute()
const selected_version = ref<CrateVersionData>(defaultCrateVersionData)
const tab = ref<CrateTab>("about");
const store = useStore();
const { mdAndUp } = useDisplay();
const docsEnabled = ref(false);
const pageState = ref<PageState>({ kind: "loading" });
// Counts the changes in the settings tab, so the access badge reloads
const accessVersion = ref(0);

const notification = useNotification();

// Expose crate as computed for template compatibility
const crate = computed(() => crateData.value);

const requestedName = computed(() => route.query.name?.toString() ?? "");
const requestedVersion = computed(() => route.query.version?.toString());

const notFoundMessage = computed(() => {
  if (pageState.value.kind === "not-found" && pageState.value.versionMissing) {
    return `Version "${requestedVersion.value}" of the crate "${requestedName.value}" was not found.`;
  }
  return requestedName.value
    ? `The crate "${requestedName.value}" was not found.`
    : "No crate was requested.";
});

const versionItems = computed(() => versionSelectItems(crateData.value.versions));

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

// Opens another version and stays on the same tab
function selectVersion(version: string | null) {
  if (!version || version === selected_version.value.version) return;
  router.push({ name: "Crate", query: { name: crateData.value.name, version, tab: tab.value } });
}

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

const hasDependencies = computed(() => selected_version.value.dependencies.length > 0);

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

// Admins are exempt from the read-only restriction, as in the backend
const canChangeYank = computed(() =>
  canManageCrate.value && (store.loggedInUserIsAdmin || !store.loggedInUserIsReadOnly)
);

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
    .then(() => notification.showSuccess("Copied to clipboard"))
    .catch(() => notification.showError("Failed to copy. Please try again."));
}

function openDocsPage() {
  if (docLink.value) {
    // Never give the docs page a handle to the UI window.
    window.open(docLink.value, "_blank", "noopener,noreferrer");
  } else {
    router.push({ name: "PublishDocs" })
  }
}

async function getCrateData(name: string, version: string | undefined) {
  const outcome = crateLoadOutcome(await crateService.getCrateData(name))
  if (outcome.kind === "error") {
    pageState.value = { kind: "error", message: outcome.message }
    setPageTitle(name)
    return
  }
  if (outcome.kind === "not-found") {
    showNotFound(false)
    return
  }

  const selected = findVersion(outcome.crate, version)
  if (selected === undefined) {
    showNotFound(true)
    return
  }
  crateData.value = outcome.crate
  selected_version.value = selected
  pageState.value = { kind: "found" }
  tab.value = tabFromRoute()
  setPageTitle(`${outcome.crate.name} ${selected.version}`)
}

function showNotFound(versionMissing: boolean) {
  crateData.value = defaultCrateData
  selected_version.value = defaultCrateVersionData
  pageState.value = { kind: "not-found", versionMissing }
  setPageTitle(versionMissing ? "Version not found" : "Crate not found")
}

// Reloads the data in place, for example after a version was yanked
async function refreshCrateData() {
  await getCrateData(requestedName.value, requestedVersion.value)
}

async function getAllData() {
  const name = requestedName.value;

  if (name === "") {
    showNotFound(false);
  } else {
    // Another version of the same crate keeps the page instead of a spinner
    if (name !== crateData.value.name) {
      pageState.value = { kind: "loading" };
    }
    await getCrateData(name, requestedVersion.value);
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

/* At least 36px high, so the chips are easy to hit */
.tag-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-height: 36px;
  padding: 4px 12px;
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

.version-select {
  flex: 0 0 auto;
  width: 200px;
}

.crate-state {
  max-width: 560px;
  margin: 48px auto;
}

.install-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--kellnr-radius-card);
}

/* The tabs sit on a line that spans the page. While scrolling they stick
   below the 64px app bar, on an opaque background. */
.crate-tabs-bar {
  position: sticky;
  top: 64px;
  z-index: 5;
  background: rgb(var(--v-theme-background));
}

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
