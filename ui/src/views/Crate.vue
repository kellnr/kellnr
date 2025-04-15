<template>
  <v-container v-if="crate != null" class="pa-0">
    <!-- Title Section -->
    <v-card class="mb-4" elevation="1">
      <v-card-title class="d-flex flex-wrap align-baseline">
        <h1 class="text-h3 font-weight-bold me-3 text-break">{{ crate.name }}</h1>
        <span class="text-h5">{{ selected_version.version }}</span>
      </v-card-title>

      <v-card-text v-if="crate.description != null">
        <p>{{ crate.description }}</p>
      </v-card-text>
    </v-card>

    <!-- Tab Navigation -->
    <v-card class="mb-4" elevation="1">
      <v-tabs v-model="tab" color="primary" grow slider-color="primary">
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
        <!-- All content tabs remain unchanged -->
        <!-- Readme Tab -->
        <v-card v-if="tab === 'readme'" class="mb-4" elevation="1">
          <v-card-text>
            <Readme :readme="selected_version.readme"></Readme>
          </v-card-text>
        </v-card>

        <!-- Versions Tab -->
        <v-card v-if="tab === 'versions'" class="mb-4" elevation="1">
          <v-card-text>
            <Version v-for="version in crate.versions" :key="version.version" :name="crate.name"
              :version="version.version" :last_updated="version.created" :downloads="version.downloads.toString()" />
          </v-card-text>
        </v-card>

        <!-- Dependencies Tab -->
        <template v-if="tab === 'deps'">
          <!-- Normal Dependencies -->
          <v-card v-if="sortedDeps.length > 0" class="mb-4" elevation="1">
            <v-card-title>Dependencies</v-card-title>
            <v-card-text>
              <Dependency v-for="dep in sortedDeps" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" />
            </v-card-text>
          </v-card>

          <!-- Dev Dependencies -->
          <v-card v-if="sortedDevDeps.length > 0" class="mb-4" elevation="1">
            <v-card-title>Development Dependencies</v-card-title>
            <v-card-text>
              <Dependency v-for="dep in sortedDevDeps" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" />
            </v-card-text>
          </v-card>

          <!-- Build Dependencies -->
          <v-card v-if="sortedBuildDeps.length > 0" class="mb-4" elevation="1">
            <v-card-title>Build Dependencies</v-card-title>
            <v-card-text>
              <Dependency v-for="dep in sortedBuildDeps" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" :desc="dep.description" />
            </v-card-text>
          </v-card>
        </template>

        <!-- About (Meta) Tab -->
        <v-card v-if="tab === 'meta'" class="mb-4" elevation="1">
          <!-- Meta content remains unchanged -->
          <!-- ... -->
        </v-card>

        <!-- Settings Tab -->
        <template v-if="tab === 'crateSettings'">
          <!-- Settings content remains unchanged -->
          <!-- ... -->
        </template>

        <!-- Admin Tab -->
        <v-card v-if="tab === 'administrate'" class="mb-4" elevation="1">
          <!-- Admin content remains unchanged -->
          <!-- ... -->
        </v-card>
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
import CrateSidebar from "../components/CrateSidebar.vue"; // Import the new sidebar component
import { computed, onBeforeMount, ref, watch } from "vue";
import axios from "axios";
import { useRoute, useRouter } from "vue-router";
import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";
import utc from "dayjs/plugin/utc";
import { defaultCrateData, defaultCrateAccessData, defaultCrateVersionData } from "../types/crate_data";
import type { CrateData, CrateAccessData, CrateVersionData, CrateRegistryDep } from "../types/crate_data";
import { CRATE_DATA, CRATE_DELETE_VERSION, CRATE_DELETE_ALL, DOCS_BUILD, CRATE_USERS, CRATE_USER, CRATE_GROUPS, CRATE_GROUP, CRATE_ACCESS_DATA } from "../remote-routes";
import { useStore } from "../store/store";

dayjs.extend(relativeTime);
dayjs.extend(utc);

const crate = ref<CrateData>(defaultCrateData);
const router = useRouter()
const route = useRoute()
const selected_version = ref<CrateVersionData>(defaultCrateVersionData)
const defaultTab = ref<string>("meta")
const tab = ref(defaultTab);
const store = useStore();

// Snackbar refs
const showSnackbar = ref(false);
const snackbarText = ref('');

const crate_access = ref<CrateAccessData>(defaultCrateAccessData);
const is_download_restricted = ref(false);
const crateUsers = ref([])
const crateUserName = ref("")
const addCrateUserStatus = ref("")
const addCrateUserMsg = ref("")
const deleteCrateUserStatus = ref("")
const deleteCrateUserMsg = ref("")
const changeCrateAccessStatus = ref("")
const changeCrateAccessMsg = ref("")
const crateGroups = ref([])
const crateGroupName = ref("")
const addCrateGroupStatus = ref("")
const addCrateGroupMsg = ref("")
const deleteCrateGroupStatus = ref("")
const deleteCrateGroupMsg = ref("")

const docLink = computed(() => {
  return selected_version.value.documentation;
})

const humanizedLastUpdated = computed(() => {
  return dayjs.utc(crate.value.last_updated).fromNow();
})

const sortedDeps = computed(() => {
  const normalDeps = selected_version.value.dependencies.filter((dep: CrateRegistryDep) => {
    return dep.kind == "normal";
  });
  return sortByName(normalDeps);
});

const sortedDevDeps = computed(() => {
  const devDeps = selected_version.value.dependencies.filter((dep: CrateRegistryDep) => {
    return dep.kind == "dev";
  });

  return sortByName(devDeps);
});

const sortedBuildDeps = computed(() => {
  const buildDeps = selected_version.value.dependencies.filter((dep: CrateRegistryDep) => {
    return dep.kind == "build";
  });

  return sortByName(buildDeps);
});

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
  const users = crate.value.owners ?? [];
  return users.sort();
});

// Keep all the functions related to API calls and user interactions
function addCrateUser() {
  // Function implementation remains unchanged
}

function deleteCrateUser(name: string) {
  // Function implementation remains unchanged
}

function getCrateUsers() {
  // Function implementation remains unchanged
}

function addCrateGroup() {
  // Function implementation remains unchanged
}

function deleteCrateGroup(name: string) {
  // Function implementation remains unchanged
}

function getCrateGroups() {
  // Function implementation remains unchanged
}

function deleteVersion(crate: string, version: string) {
  // Function implementation remains unchanged
}

function deleteCrate(crate: string) {
  // Function implementation remains unchanged
}

function showBuildRustdoc(): boolean {
  // Show the option to build the docs, if the current logged-in user is and admin
  if (store.loggedInUserIsAdmin) {
    return true
  }

  // Show the option to build the docs, if the current logged-in user owns the crate
  return crate.value.owners.includes(store.loggedInUser);
}

function buildDoc(crate: string, version: string) {
  axios.post(DOCS_BUILD, null, { params: { package: crate, version: version } })
    .then(() => {
      router.push({ name: "DocQueue" })
    })
    .catch((error) => {
      console.log(error)
    })
}

function changeTab(newTab: string) {
  if (newTab === "crateSettings") {
    getCrateAccessData();
    getCrateUsers();
  }
  tab.value = newTab;
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
  const text = crate.value.name + ' = "' + selected_version.value.version + '"';
  navigator.clipboard.writeText(text)
    .then(() => {
      // Show success snackbar
      snackbarText.value = "Copied to clipboard!";
      showSnackbar.value = true;
    })
    .catch(() => {
      // Show error snackbar if copying fails
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

function getCrateData(name: string, version?: string) {
  axios
    .get(CRATE_DATA, { params: { name: name } })
    .then((response) => {
      crate.value = response.data;
      version = version ?? crate.value.max_version;
      selected_version.value = crate.value.versions.find((cvd: CrateVersionData) => {
        return cvd.version == version;
      }) ?? defaultCrateVersionData;

      // Set the default tab to "readme" if a readme is available, else "meta"
      defaultTab.value = selected_version.value.readme == null ? "meta" : "readme";
      tab.value = defaultTab.value;
    })
    .catch((error) => {
      console.log(error);
    });
}

function getCrateAccessData() {
  // Function implementation remains unchanged
}

function setCrateAccessData() {
  // Function implementation remains unchanged
}

function getAllData() {
  const version = route.query.version?.toString();
  const name = route.query.name?.toString() ?? "";

  if (name !== "") {
    getCrateData(name, version);
  }
}

onBeforeMount(() => {
  getAllData()
})

// Watches route changes and reloads the data.
watch(route, () => {
  getAllData()
})

// Watch for tab changes to load data when needed
watch(tab, (newTab) => {
  if (newTab === 'crateSettings') {
    getCrateAccessData();
    getCrateUsers();
    getCrateGroups();
  }
})
</script>

<style>
.text-break {
  word-break: break-word;
}
</style>
