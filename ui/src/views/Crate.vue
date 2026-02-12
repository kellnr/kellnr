<template>
  <v-container v-if="crate != null" class="pa-0 crate-container">
    <!-- Title Section -->
    <v-card class="mb-4 title-card" elevation="0">
      <v-card-title class="d-flex flex-wrap align-baseline pa-5">
        <h1 class="text-h3 font-weight-bold me-3 text-break crate-title">{{ crate.name }}</h1>
        <span class="text-h5 version-text">{{ selected_version.version }}</span>
      </v-card-title>

      <v-card-text v-if="crate.description != null" class="pt-0 px-5 pb-5">
        <p class="description-text mb-0">{{ crate.description }}</p>
      </v-card-text>
    </v-card>

    <!-- Tab Navigation -->
    <v-card class="mb-4 tabs-card" elevation="0">
      <v-tabs v-model="tab" color="primary" grow slider-color="primary" class="crate-tabs">
        <v-tab v-if="selected_version.readme" value="readme">Readme</v-tab>
        <v-tab value="meta">About</v-tab>
        <v-tab value="deps">Dependencies</v-tab>
        <v-tab value="versions">Versions</v-tab>
        <v-tab v-if="store.loggedInUserIsAdmin" value="crateSettings">Settings</v-tab>
        <v-tab v-if="store.loggedInUserIsAdmin" value="administrate">Admin</v-tab>
      </v-tabs>
    </v-card>

    <!-- Snackbar for copy notification -->
    <v-snackbar v-model="showSnackbar" :timeout="3000" color="success" location="bottom">
      {{ snackbarText }}
      <template v-slot:actions>
        <v-btn variant="text" icon="mdi-close" @click="showSnackbar = false"></v-btn>
      </template>
    </v-snackbar>

    <v-row>
      <!-- Main Content Area -->
      <v-col cols="12" md="9" order="2" order-md="1">
        <!-- Readme Tab -->
        <v-card v-if="tab === 'readme'" class="mb-4 content-card" elevation="0">
          <Readme :readme="selected_version.readme"></Readme>
        </v-card>

        <!-- Versions Tab -->
        <v-card v-if="tab === 'versions'" class="mb-4 content-card" elevation="0">
          <v-card-text>
            <Version v-for="version in crate.versions" :key="version.version" :name="crate.name"
              :version="version.version" :last_updated="version.created" :downloads="version.downloads.toString()" />
          </v-card-text>
        </v-card>

        <!-- Dependencies Tab -->
        <template v-if="tab === 'deps'">
          <!-- Normal Dependencies -->
          <v-card v-if="sortedDependencies.normal.length > 0" class="mb-4 content-card" elevation="0">
            <v-card-title>Dependencies</v-card-title>
            <v-card-text>
              <Dependency v-for="dep in sortedDependencies.normal" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" />
            </v-card-text>
          </v-card>

          <!-- Dev Dependencies -->
          <v-card v-if="sortedDependencies.dev.length > 0" class="mb-4 content-card" elevation="0">
            <v-card-title>Development Dependencies</v-card-title>
            <v-card-text>
              <Dependency v-for="dep in sortedDependencies.dev" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" />
            </v-card-text>
          </v-card>

          <!-- Build Dependencies -->
          <v-card v-if="sortedDependencies.build.length > 0" class="mb-4 content-card" elevation="0">
            <v-card-title>Build Dependencies</v-card-title>
            <v-card-text>
              <Dependency v-for="dep in sortedDependencies.build" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" :desc="dep.description" />
            </v-card-text>
          </v-card>
        </template>

        <!-- About (Meta) Tab -->
        <v-card v-if="tab === 'meta'" class="mb-4 content-card" elevation="0">
          <About :crate="crate" :selected-version="selected_version" :flattened-features="flattenedFeatures"
            :sorted-owners="sortedOwners" />
        </v-card>

        <!-- Settings Tab -->
        <CrateSettingsTab
          v-if="tab === 'crateSettings'"
          :crate-name="crate.name"
          @owners-changed="handleOwnersChanged"
        />

        <!-- Admin Tab -->
        <CrateAdminTab
          v-if="tab === 'administrate'"
          :crate-name="crate.name"
          :version="selected_version.version"
        />
      </v-col>

      <!-- Sidebar Column - Now using the CrateSidebar component -->
      <v-col cols="12" md="3" order="1" order-md="2">
        <CrateSidebar :crate-name="crate.name" :version="selected_version.version" :last-updated="crate.last_updated"
          :humanized-last-updated="humanizedLastUpdated" :version-downloads="selected_version.downloads"
          :total-downloads="crate.total_downloads" :documentation-link="docLink" :can-build-docs="showBuildRustdoc()"
          @copy-to-clipboard="copyTomlToClipboard" @open-docs="openDocsPage"
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
import { computed, onBeforeMount, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";
import utc from "dayjs/plugin/utc";
import { defaultCrateData, defaultCrateVersionData } from "../types/crate_data";
import type { CrateData, CrateVersionData, CrateRegistryDep } from "../types/crate_data";
import { crateService, settingsService } from "../services";
import { isSuccess } from "../services/api";
import { useStore } from "../store/store";

dayjs.extend(relativeTime);
dayjs.extend(utc);

const crateData = ref<CrateData>(defaultCrateData);
const router = useRouter()
const route = useRoute()
const selected_version = ref<CrateVersionData>(defaultCrateVersionData)
const defaultTab = ref<string>("meta")
const tab = ref(defaultTab.value);
const store = useStore();
const docsEnabled = ref(false);

// Snackbar refs
const showSnackbar = ref(false);
const snackbarText = ref('');

// Expose crate as computed for template compatibility
const crate = computed(() => crateData.value);

const docLink = computed(() => {
  return selected_version.value.documentation;
})

const humanizedLastUpdated = computed(() => {
  return dayjs.utc(crateData.value.last_updated).fromNow();
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

function showBuildRustdoc(): boolean {
  if (!docsEnabled.value) {
    return false;
  }
  // Show the option to build the docs, if the current logged-in user is admin
  if (store.loggedInUserIsAdmin) {
    return true
  }
  // Show the option to build the docs, if the current logged-in user owns the crate
  return crateData.value.owners.includes(store.loggedInUser);
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

function copyTomlToClipboard() {
  const text = crateData.value.name + ' = "' + selected_version.value.version + '"';
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
  if (selected_version.value.documentation) {
    let url = selected_version.value.documentation;
    window.open(url, "_blank");
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

    // Set the default tab to "readme" if a readme is available, else "meta"
    defaultTab.value = selected_version.value.readme == null ? "meta" : "readme";
    tab.value = defaultTab.value;
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

// Watches route changes and reloads the data.
// Needed, if the query parameter "name=crate" changes.
watch(route, () => {
  getAllData()
})

</script>

<style scoped>
.crate-container {
  max-width: 1400px;
  margin: 0 auto;
}

.text-break {
  word-break: break-word;
}

/* Title Card */
.title-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
}

.crate-title {
  color: rgb(var(--v-theme-on-surface));
}

.version-text {
  color: rgb(var(--v-theme-on-surface-variant));
}

.description-text {
  color: rgb(var(--v-theme-on-surface-variant));
  font-size: 1rem;
  line-height: 1.6;
}

/* Tabs Card */
.tabs-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  overflow: hidden;
}

.crate-tabs {
  background: transparent;
}

.crate-tabs :deep(.v-tab) {
  text-transform: uppercase;
  font-weight: 500;
  letter-spacing: 0.5px;
}

/* Content Cards */
.content-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
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
  border-radius: 8px;
}

:deep(.v-list-item:hover) {
  background: rgb(var(--v-theme-surface-variant));
}

/* Alerts */
:deep(.v-alert) {
  border-radius: 8px;
}

/* Form fields */
:deep(.v-text-field) {
  margin-bottom: 8px;
}
</style>
