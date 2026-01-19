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
          <v-card v-if="sortedDeps.length > 0" class="mb-4 content-card" elevation="0">
            <v-card-title>Dependencies</v-card-title>
            <v-card-text>
              <Dependency v-for="dep in sortedDeps" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" />
            </v-card-text>
          </v-card>

          <!-- Dev Dependencies -->
          <v-card v-if="sortedDevDeps.length > 0" class="mb-4 content-card" elevation="0">
            <v-card-title>Development Dependencies</v-card-title>
            <v-card-text>
              <Dependency v-for="dep in sortedDevDeps" :key="dep.name" :name="dep.name" :version="dep.version_req"
                :registry="dep.registry" />
            </v-card-text>
          </v-card>

          <!-- Build Dependencies -->
          <v-card v-if="sortedBuildDeps.length > 0" class="mb-4 content-card" elevation="0">
            <v-card-title>Build Dependencies</v-card-title>
            <v-card-text>
              <Dependency v-for="dep in sortedBuildDeps" :key="dep.name" :name="dep.name" :version="dep.version_req"
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
        <template v-if="tab === 'crateSettings'">
          <!-- Crate Owners -->
          <v-card class="mb-4 content-card settings-card" elevation="0">
            <div class="settings-header">
              <v-icon icon="mdi-shield-account" size="small" class="settings-icon"></v-icon>
              <span class="settings-title">Crate Owners</span>
              <span class="settings-count">{{ crateOwners.length }}</span>
            </div>
            <v-card-text class="settings-content">
              <v-alert v-if="!canManageOwners()" type="info" variant="tonal" class="mb-4">
                Only existing crate owners or admins can add/remove crate owners.
              </v-alert>

              <div v-if="crateOwners.length > 0" class="settings-list">
                <div v-for="owner in crateOwners" :key="owner.login" class="settings-list-item">
                  <div class="settings-list-item-info">
                    <v-icon icon="mdi-account" size="small" class="me-3"></v-icon>
                    <span class="settings-list-item-name">{{ owner.login }}</span>
                  </div>
                  <v-btn :disabled="!canManageOwners()" color="error" variant="tonal" size="small"
                    @click="deleteCrateOwner(owner.login)">
                    <v-icon icon="mdi-delete-outline" size="small" class="me-1"></v-icon>
                    Remove
                  </v-btn>
                </div>
              </div>
              <p v-else class="text-body-2 text-disabled mb-0">No owners assigned yet.</p>

              <v-alert v-if="deleteCrateOwnerStatus" :type="deleteCrateOwnerStatus === 'Success' ? 'success' : 'error'"
                closable @click:close="deleteCrateOwnerStatus = ''" class="mt-4">
                {{ deleteCrateOwnerMsg }}
              </v-alert>

              <div class="settings-form-section">
                <div class="settings-form-header">
                  <v-icon icon="mdi-account-plus" size="small" class="me-2"></v-icon>
                  <span>Add crate owner</span>
                </div>
                <v-form @submit.prevent="addCrateOwner" class="settings-form">
                  <v-text-field v-model="crateOwnerName" placeholder="Enter username" prepend-inner-icon="mdi-account-star"
                    variant="outlined" density="comfortable" :disabled="!canManageOwners()" hide-details class="settings-input"></v-text-field>
                  <v-btn :disabled="!canManageOwners()" color="primary" type="submit" variant="flat">
                    <v-icon icon="mdi-plus" size="small" class="me-1"></v-icon>
                    Add
                  </v-btn>
                </v-form>
                <v-alert v-if="addCrateOwnerStatus" :type="addCrateOwnerStatus === 'Success' ? 'success' : 'error'"
                  closable @click:close="addCrateOwnerStatus = ''" class="mt-3">
                  {{ addCrateOwnerMsg }}
                </v-alert>
              </div>
            </v-card-text>
          </v-card>

          <!-- Access Control -->
          <v-card class="mb-4 content-card settings-card" elevation="0">
            <div class="settings-header">
              <v-icon icon="mdi-lock-outline" size="small" class="settings-icon"></v-icon>
              <span class="settings-title">Access Control</span>
            </div>
            <v-card-text class="settings-content">
              <!-- Crate Access Rules -->
              <div class="settings-subsection">
                <div class="settings-subsection-header">
                  <v-icon icon="mdi-shield-lock" size="x-small" class="me-2"></v-icon>
                  <span>Download Restrictions</span>
                </div>
                <v-form @submit.prevent="setCrateAccessData">
                  <v-checkbox v-model="is_download_restricted" hide-details
                    label="Restrict downloads to crate users only" class="settings-checkbox"></v-checkbox>
                  <p class="settings-help-text">
                    When enabled, only users explicitly added as crate users can download this crate.
                    Requires <code>auth_required = true</code> in kellnr configuration.
                  </p>
                  <v-alert v-if="changeCrateAccessStatus"
                    :type="changeCrateAccessStatus === 'Success' ? 'success' : 'error'" closable
                    @click:close="changeCrateAccessStatus = ''" class="mb-3">
                    {{ changeCrateAccessMsg }}
                  </v-alert>
                  <v-btn color="primary" type="submit" variant="flat" size="small">
                    <v-icon icon="mdi-content-save" size="small" class="me-1"></v-icon>
                    Save Changes
                  </v-btn>
                </v-form>
              </div>

              <!-- Crate Users -->
              <div class="settings-subsection">
                <div class="settings-subsection-header">
                  <v-icon icon="mdi-account-multiple" size="x-small" class="me-2"></v-icon>
                  <span>Crate Users</span>
                  <span class="settings-count-small">{{ crateUsers.length }}</span>
                </div>

                <div v-if="crateUsers.length > 0" class="settings-list compact">
                  <div v-for="user in crateUsers" :key="user.login" class="settings-list-item">
                    <div class="settings-list-item-info">
                      <v-icon icon="mdi-account" size="small" class="me-3"></v-icon>
                      <span class="settings-list-item-name">{{ user.login }}</span>
                    </div>
                    <v-btn color="error" variant="text" size="small" @click="deleteCrateUser(user.login)">
                      <v-icon icon="mdi-close" size="small"></v-icon>
                    </v-btn>
                  </div>
                </div>
                <p v-else class="text-body-2 text-disabled mb-4">No users assigned yet.</p>

                <v-alert v-if="deleteCrateUserStatus"
                  :type="deleteCrateUserStatus === 'Success' ? 'success' : 'error'" closable
                  @click:close="deleteCrateUserStatus = ''" class="mb-3">
                  {{ deleteCrateUserMsg }}
                </v-alert>

                <v-form @submit.prevent="addCrateUser" class="settings-form inline">
                  <v-text-field v-model="crateUserName" placeholder="Enter username" prepend-inner-icon="mdi-account"
                    variant="outlined" density="compact" hide-details class="settings-input"></v-text-field>
                  <v-btn color="primary" type="submit" variant="tonal" size="small">
                    <v-icon icon="mdi-plus" size="small"></v-icon>
                  </v-btn>
                </v-form>
                <v-alert v-if="addCrateUserStatus" :type="addCrateUserStatus === 'Success' ? 'success' : 'error'"
                  closable @click:close="addCrateUserStatus = ''" class="mt-3">
                  {{ addCrateUserMsg }}
                </v-alert>
              </div>

              <!-- Crate Groups -->
              <div class="settings-subsection last">
                <div class="settings-subsection-header">
                  <v-icon icon="mdi-account-group" size="x-small" class="me-2"></v-icon>
                  <span>Crate Groups</span>
                  <span class="settings-count-small">{{ crateGroupsForCrate.length }}</span>
                </div>

                <div v-if="crateGroupsForCrate.length > 0" class="settings-list compact">
                  <div v-for="group in crateGroupsForCrate" :key="group.name" class="settings-list-item">
                    <div class="settings-list-item-info">
                      <v-icon icon="mdi-account-group" size="small" class="me-3"></v-icon>
                      <span class="settings-list-item-name">{{ group.name }}</span>
                    </div>
                    <v-btn color="error" variant="text" size="small" @click="deleteCrateGroup(group.name)">
                      <v-icon icon="mdi-close" size="small"></v-icon>
                    </v-btn>
                  </div>
                </div>
                <p v-else class="text-body-2 text-disabled mb-4">No groups assigned yet.</p>

                <v-alert v-if="deleteCrateGroupStatus"
                  :type="deleteCrateGroupStatus === 'Success' ? 'success' : 'error'" closable
                  @click:close="deleteCrateGroupStatus = ''" class="mb-3">
                  {{ deleteCrateGroupMsg }}
                </v-alert>

                <v-form @submit.prevent="addCrateGroup" class="settings-form inline">
                  <v-select v-model="crateGroupName" :items="availableCrateGroups" label="Select group"
                    prepend-inner-icon="mdi-account-group" variant="outlined" density="compact" hide-details class="settings-input" />
                  <v-btn color="primary" type="submit" variant="tonal" size="small">
                    <v-icon icon="mdi-plus" size="small"></v-icon>
                  </v-btn>
                </v-form>
                <v-alert v-if="addCrateGroupStatus" :type="addCrateGroupStatus === 'Success' ? 'success' : 'error'"
                  closable @click:close="addCrateGroupStatus = ''" class="mt-3">
                  {{ addCrateGroupMsg }}
                </v-alert>
              </div>
            </v-card-text>
          </v-card>
        </template>

        <!-- Admin Tab -->
        <v-card v-if="tab === 'administrate'" class="mb-4 content-card admin-card" elevation="0">
          <div class="admin-header">
            <v-icon icon="mdi-shield-alert" size="small" class="admin-icon"></v-icon>
            <span class="admin-title">Danger Zone</span>
          </div>
          <v-card-text class="admin-content">
            <v-alert type="warning" variant="tonal" class="mb-5 admin-alert">
              <div class="admin-alert-content">
                <v-icon icon="mdi-lightbulb-outline" size="small" class="me-2"></v-icon>
                <span>
                  Consider <a href="https://doc.rust-lang.org/cargo/commands/cargo-yank.html" target="_blank" class="admin-link">yanking</a>
                  the crate instead of deleting it. Yanking prevents new dependencies but doesn't break existing ones.
                </span>
              </div>
            </v-alert>

            <div class="admin-actions">
              <!-- Delete Version -->
              <div class="admin-action-card destructive">
                <div class="admin-action-info">
                  <div class="admin-action-header">
                    <v-icon icon="mdi-tag-remove" size="small" class="admin-action-icon"></v-icon>
                    <span class="admin-action-title">Delete Version</span>
                  </div>
                  <p class="admin-action-desc">
                    Permanently delete version <code>{{ selected_version.version }}</code> of this crate.
                    This action cannot be undone.
                  </p>
                </div>
                <v-btn color="error" variant="flat" @click="deleteVersion(crate.name, selected_version.version)">
                  <v-icon icon="mdi-delete-outline" size="small" class="me-2"></v-icon>
                  Delete Version
                </v-btn>
              </div>

              <!-- Delete Entire Crate -->
              <div class="admin-action-card destructive">
                <div class="admin-action-info">
                  <div class="admin-action-header">
                    <v-icon icon="mdi-delete-forever" size="small" class="admin-action-icon"></v-icon>
                    <span class="admin-action-title">Delete Entire Crate</span>
                  </div>
                  <p class="admin-action-desc">
                    Permanently delete <strong>all versions</strong> of <code>{{ crate.name }}</code>.
                    This will break all crates that depend on it.
                  </p>
                </div>
                <v-btn color="error" variant="flat" @click="deleteCrate(crate.name)">
                  <v-icon icon="mdi-delete-forever" size="small" class="me-2"></v-icon>
                  Delete Crate
                </v-btn>
              </div>
            </div>
          </v-card-text>
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
import CrateSidebar from "../components/CrateSidebar.vue";
import About from "../components/About.vue";
import { computed, onBeforeMount, ref, watch } from "vue";
import axios from "axios";
import { useRoute, useRouter } from "vue-router";
import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";
import utc from "dayjs/plugin/utc";
import { defaultCrateData, defaultCrateAccessData, defaultCrateVersionData } from "../types/crate_data";
import type { CrateData, CrateAccessData, CrateVersionData, CrateRegistryDep, CrateGroup } from "../types/crate_data";
import { CRATE_DATA, CRATE_DELETE_VERSION, CRATE_DELETE_ALL, DOCS_BUILD, CRATE_USERS, CRATE_USER, CRATE_GROUPS, CRATE_GROUP, CRATE_ACCESS_DATA, LIST_GROUPS, CRATE_OWNERS, CRATE_OWNER } from "../remote-routes";

import { useStore } from "../store/store";

dayjs.extend(relativeTime);
dayjs.extend(utc);

const crate = ref<CrateData>(defaultCrateData);
const router = useRouter()
const route = useRoute()
const selected_version = ref<CrateVersionData>(defaultCrateVersionData)
const defaultTab = ref<string>("meta")
const tab = ref(defaultTab.value);
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

// Owners management
const crateOwners = ref<{ login: string }[]>([])
const crateOwnerName = ref("")
const addCrateOwnerStatus = ref("")
const addCrateOwnerMsg = ref("")
const deleteCrateOwnerStatus = ref("")
const deleteCrateOwnerMsg = ref("")

const crateGroupsForCrate = ref<CrateGroup[]>([])
const crateGroups = ref<CrateGroup[]>([])
const crateGroupName = ref("")


const availableCrateGroups = computed(() => {
  const assigned = new Set(crateGroupsForCrate.value.map((g) => g.name))
  return crateGroups.value.filter((group) => !assigned.has(group.name)).map((group) => group.name)
})
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

function addCrateUser() {
  axios
    .put(CRATE_USER(crate.value.name, crateUserName.value), null, { withCredentials: true })

    .then((res) => {
      if (res.status == 200) {
        addCrateUserStatus.value = "Success";
        addCrateUserMsg.value = "Crate user successfully added.";
        // Update user list
        getCrateUsers();
      }
    })
    .catch((error) => {
      if (error.response) {
        addCrateUserStatus.value = "Error";
        addCrateUserMsg.value = "Crate user could not be added.";

        if (error.response.status == 401 || error.response.status == 403) {
          // "Unauthorized. Login first."
          router.push("/login");
        }
        else if (error.response.status == 404) {
          addCrateUserMsg.value = "User not found. Did you provide an existing user name?";
        } else if (error.response.status == 500) {
          addCrateUserMsg.value = "Crate user could not be added";
        } else {
          addCrateUserMsg.value = "Unknown error";
        }
      }
    });
}

function deleteCrateUser(name: string) {
  if (confirm('Delete crate user "' + name + '"?')) {
    axios
      .delete(CRATE_USER(crate.value.name, name), { withCredentials: true })

      .then((res) => {
        if (res.status == 200) {
          deleteCrateUserStatus.value = "Success";
          deleteCrateUserMsg.value = "Crate user successfully deleted.";
          // Update user list
          getCrateUsers();
        }
      })
      .catch((error) => {
        if (error.response) {
          deleteCrateUserStatus.value = "Error";
          deleteCrateUserMsg.value = "Crate user could not be deleted.";

          if (error.response.status == 404) {
            // "Unauthorized. Login first."
            router.push("/login");
          } else if (error.response.status == 500) {
            deleteCrateUserMsg.value = "Crate user could not be deleted";
          } else {
            deleteCrateUserMsg.value = "Unknown error";
          }
        }
      });
  }
}

function getCrateUsers() {
  // disable caching to get updated token list
  axios
    // @ts-expect-error TS doesn't recognize cache option
    .get(CRATE_USERS(crate.value.name), { cache: false, withCredentials: true })

    .then((res) => {
      if (res.status == 200) {
        crateUsers.value = res.data.users;
      }
    })
    .catch((error) => {
      console.log(error);
    });
}

function canManageOwners(): boolean {
  return store.loggedInUserIsAdmin || (crateOwners.value ?? []).some((o) => o.login === store.loggedInUser)
}

function getCrateOwners() {
  axios
    // @ts-expect-error TS doesn't recognize cache option
    .get(CRATE_OWNERS(crate.value.name), { cache: false, withCredentials: true })

    .then((res) => {
      if (res.status == 200) {
        crateOwners.value = res.data.users ?? []

        // Keep crate metadata in sync so the About tab and other owner-based UI (e.g. Build docs)
        // updates immediately without a full crate reload.
        crate.value.owners = (crateOwners.value ?? []).map((o) => o.login)
      }
    })
    .catch((error) => {
      console.log(error)
      crateOwners.value = []
      crate.value.owners = []
    })
}


function addCrateOwner() {
  if (!canManageOwners()) {
    addCrateOwnerStatus.value = "Error"
    addCrateOwnerMsg.value = "Not allowed. Only existing owners or admins can add owners."
    return
  }

  axios
    .put(CRATE_OWNER(crate.value.name, crateOwnerName.value), null, { withCredentials: true })

    .then((res) => {
      if (res.status == 200) {
        addCrateOwnerStatus.value = "Success"
        addCrateOwnerMsg.value = "Crate owner successfully added."
        crateOwnerName.value = ""
        getCrateOwners()

      }
    })
    .catch((error) => {
      if (error.response) {
        addCrateOwnerStatus.value = "Error"

        if (error.response.status == 401) {
          addCrateOwnerMsg.value = "Unauthorized. Login first."
          router.push("/login")
        } else if (error.response.status == 403) {
          addCrateOwnerMsg.value = "Not allowed. Only existing owners or admins can add owners."
        } else if (error.response.status == 404) {
          addCrateOwnerMsg.value = "User not found. Did you provide an existing user name?"
        } else {
          addCrateOwnerMsg.value = "Crate owner could not be added."
        }
      }
    })

}

function deleteCrateOwner(name: string) {
  if (!canManageOwners()) {
    deleteCrateOwnerStatus.value = "Error"
    deleteCrateOwnerMsg.value = "Not allowed. Only existing owners or admins can remove owners."
    return
  }

  // Client-side guard: never allow removing the last owner.
  if ((crateOwners.value?.length ?? 0) <= 1) {
    deleteCrateOwnerStatus.value = "Error"
    deleteCrateOwnerMsg.value = "A crate must have at least one owner."
    return
  }

  if (confirm('Delete crate owner "' + name + '"?')) {
    axios
      .delete(CRATE_OWNER(crate.value.name, name), { withCredentials: true })

      .then((res) => {
        if (res.status == 200) {
          deleteCrateOwnerStatus.value = "Success"
          deleteCrateOwnerMsg.value = "Crate owner successfully deleted."
          getCrateOwners()

        }
      })
      .catch((error) => {
        if (error.response) {
          deleteCrateOwnerStatus.value = "Error"
          if (error.response.status == 401) {
            deleteCrateOwnerMsg.value = "Unauthorized. Login first."
            router.push("/login")
          } else if (error.response.status == 403) {
            deleteCrateOwnerMsg.value = "Not allowed. Only existing owners or admins can remove owners."
          } else if (error.response.status == 409) {
            deleteCrateOwnerMsg.value = "A crate must have at least one owner."
          } else {
            deleteCrateOwnerMsg.value = "Crate owner could not be deleted."
          }
        }
      })
  }
}



function addCrateGroup() {
  axios
    .put(CRATE_GROUP(crate.value.name, crateGroupName.value), null, { withCredentials: true })

    .then((res) => {
      if (res.status == 200) {
        addCrateGroupStatus.value = "Success";
        addCrateGroupMsg.value = "Crate group successfully added.";
        // Update group list
        getCrateGroupsForCrate();
      }
    })
    .catch((error) => {
      if (error.response) {
        addCrateGroupStatus.value = "Error";
        addCrateGroupMsg.value = "Crate group could not be added.";

        if (error.response.status == 401 || error.response.status == 403) {
          // "Unauthorized. Login first."
          router.push("/login");
        }
        else if (error.response.status == 404) {
          addCrateGroupMsg.value = "Group not found. Did you provide an existing group name?";
        } else if (error.response.status == 500) {
          addCrateGroupMsg.value = "Crate group could not be added";
        } else {
          addCrateGroupMsg.value = "Unknown error";
        }
      }
    });
}

function deleteCrateGroup(name: string) {
  if (confirm('Delete crate group "' + name + '"?')) {
    axios
      .delete(CRATE_GROUP(crate.value.name, name), { withCredentials: true })

      .then((res) => {
        if (res.status == 200) {
          deleteCrateGroupStatus.value = "Success";
          deleteCrateGroupMsg.value = "Crate group successfully deleted.";
          // Update group list
          getCrateGroupsForCrate();
        }
      })
      .catch((error) => {
        if (error.response) {
          deleteCrateGroupStatus.value = "Error";
          deleteCrateGroupMsg.value = "Crate group could not be deleted.";

          if (error.response.status == 404) {
            // "Unauthorized. Login first."
            router.push("/login");
          } else if (error.response.status == 500) {
            deleteCrateGroupMsg.value = "Crate group could not be deleted";
          } else {
            deleteCrateGroupMsg.value = "Unknown error";
          }
        }
      });
  }
}

function getCrateGroupsForCrate() {
  // disable caching to get updated group list
  axios
    // @ts-expect-error TS doesn't recognize cache option
    .get(CRATE_GROUPS(crate.value.name), { cache: false, withCredentials: true })

    .then((res) => {
      if (res.status == 200) {
        crateGroupsForCrate.value = res.data.groups;
      }
    })
    .catch((error) => {
      console.log(error);
    });
}

async function getAllCrateGroups() {
  // disable caching to get updated group list
  try {
    // @ts-expect-error TS doesn't recognize cache option
    const res = await axios.get(LIST_GROUPS, { cache: false })
    if (res.status == 200) {
      // LIST_GROUPS returns: [{"name":"group1"},{"name":"group2"}]
      crateGroups.value = res.data ?? []
    }
  } catch (error) {
    console.log(error);
    crateGroups.value = []
  }
}

function deleteVersion(crate: string, version: string) {
  if (confirm('Delete "' + crate + '" version "' + version + '"?')) {
    axios.delete(CRATE_DELETE_VERSION,
      {
        params: {
          name: crate,
          version: version
        }
      }
    ).then(() => {
      router.push({ name: "Crates" })
    }).catch((error) => {
      console.log(error);
    });
  }
}

function deleteCrate(crate: string) {
  if (confirm('Delete all versions of "' + crate + '"?')) {
    axios.delete(CRATE_DELETE_ALL,
      {
        params: {
          name: crate,
        }
      }
    ).then(() => {
      router.push({ name: "Crates" })
    }).catch((error) => {
      console.log(error);
    });
  }
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
    getCrateOwners();
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
  // disable caching to get updated token list
  axios
    // @ts-expect-error TS doesn't recognize cache option
    .get(CRATE_ACCESS_DATA(crate.value.name), { cache: false, withCredentials: true })

    .then((response) => {
      crate_access.value = response.data;
      is_download_restricted.value = crate_access.value.download_restricted;
    })
    .catch((error) => {
      console.log(error);
    });
}

function setCrateAccessData() {
  const putData = {
    download_restricted: is_download_restricted.value,
  }

  axios
    .put(CRATE_ACCESS_DATA(crate.value.name), putData, { withCredentials: true })

    .then((res) => {
      if (res.status == 200) {
        changeCrateAccessStatus.value = "Success";

        if (res.data.download_restricted) {
          changeCrateAccessMsg.value = "Crate access restricted to crate users only.";
        } else {
          changeCrateAccessMsg.value = "Crate access open to all.";
        }

        // Update user list
        getCrateAccessData();
      }
    })
    .catch((error) => {
      if (error.response) {
        changeCrateAccessStatus.value = "Error";
        changeCrateAccessMsg.value = "Crate access data could not be changed.";

        if (error.response.status == 403 || error.response.status == 401) {
          console.log("Unauthorized. Login first.");
          // "Unauthorized. Login first."
          router.push("/login");
        }
        else if (error.response.status == 500) {
          changeCrateAccessMsg.value = "Crate access data could not be changed";
        } else {
          changeCrateAccessMsg.value = "Unknown error";
        }
      }
    });
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
// Needed, if the query parameter "name=crate" changes.
watch(route, () => {
  getAllData()
})

// Watch for tab changes to load data when needed
watch(tab, (newTab) => {
  if (newTab === 'crateSettings') {
    getCrateAccessData();
    getCrateUsers();
    getCrateOwners();
    getCrateGroupsForCrate();
    getAllCrateGroups();
  }
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

/* List items in settings */
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

/* Nested cards in settings */
.nested-card {
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
}

/* Settings Card Styles */
.settings-card {
  overflow: hidden;
}

.settings-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  background: rgb(var(--v-theme-surface-variant));
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.settings-icon {
  color: rgb(var(--v-theme-primary));
  margin-right: 12px;
}

.settings-title {
  font-weight: 600;
  font-size: 16px;
  color: rgb(var(--v-theme-on-surface));
}

.settings-count {
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface-variant));
  font-size: 12px;
  font-weight: 500;
  padding: 2px 10px;
  border-radius: 12px;
  margin-left: auto;
}

.settings-content {
  padding: 20px !important;
}

.settings-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 16px;
}

.settings-list.compact {
  margin-bottom: 12px;
}

.settings-list-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  transition: all 0.2s ease;
}

.settings-list-item:hover {
  border-color: rgb(var(--v-theme-primary));
}

.settings-list-item-info {
  display: flex;
  align-items: center;
}

.settings-list-item-info .v-icon {
  color: rgb(var(--v-theme-on-surface-variant));
}

.settings-list-item-name {
  font-weight: 500;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.settings-form-section {
  margin-top: 24px;
  padding-top: 20px;
  border-top: 1px solid rgb(var(--v-theme-outline));
}

.settings-form-header {
  display: flex;
  align-items: center;
  font-weight: 500;
  font-size: 14px;
  color: rgb(var(--v-theme-on-surface-variant));
  margin-bottom: 12px;
}

.settings-form-header .v-icon {
  color: rgb(var(--v-theme-primary));
}

.settings-form {
  display: flex;
  gap: 12px;
  align-items: flex-start;
}

.settings-form.inline {
  align-items: center;
}

.settings-input {
  flex: 1;
}

.settings-input :deep(.v-field) {
  margin-bottom: 0;
}

.settings-subsection {
  padding: 20px;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  margin-bottom: 16px;
}

.settings-subsection.last {
  margin-bottom: 0;
}

.settings-subsection-header {
  display: flex;
  align-items: center;
  font-weight: 600;
  font-size: 14px;
  color: rgb(var(--v-theme-on-surface));
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.settings-subsection-header .v-icon {
  color: rgb(var(--v-theme-primary));
}

.settings-count-small {
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface-variant));
  font-size: 11px;
  font-weight: 500;
  padding: 1px 8px;
  border-radius: 10px;
  margin-left: auto;
}

.settings-checkbox {
  margin-bottom: 8px;
}

.settings-checkbox :deep(.v-label) {
  font-size: 14px;
}

.settings-help-text {
  font-size: 13px;
  line-height: 1.5;
  color: rgb(var(--v-theme-on-surface-variant));
  margin-bottom: 16px;
  padding-left: 32px;
}

.settings-help-text code {
  background: rgb(var(--v-theme-surface));
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 12px;
  font-family: 'Roboto Mono', monospace;
}

/* Responsive */
@media (max-width: 600px) {
  .settings-form {
    flex-direction: column;
  }

  .settings-form.inline {
    flex-direction: row;
  }

  .settings-input {
    width: 100%;
  }
}

/* Admin Card Styles */
.admin-card {
  overflow: hidden;
  border-color: rgba(var(--v-theme-error), 0.3) !important;
}

.admin-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  background: rgba(var(--v-theme-error), 0.08);
  border-bottom: 1px solid rgba(var(--v-theme-error), 0.2);
}

.admin-icon {
  color: rgb(var(--v-theme-error));
  margin-right: 12px;
}

.admin-title {
  font-weight: 600;
  font-size: 16px;
  color: rgb(var(--v-theme-error));
}

.admin-content {
  padding: 20px !important;
}

.admin-alert {
  border-radius: 8px;
}

.admin-alert-content {
  display: flex;
  align-items: flex-start;
}

.admin-alert-content .v-icon {
  flex-shrink: 0;
  margin-top: 2px;
}

.admin-link {
  color: rgb(var(--v-theme-primary));
  font-weight: 500;
  text-decoration: none;
}

.admin-link:hover {
  text-decoration: underline;
}

.admin-actions {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.admin-action-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  padding: 20px;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  transition: all 0.2s ease;
}

.admin-action-card:hover {
  border-color: rgba(var(--v-theme-error), 0.5);
}

.admin-action-card.destructive {
  background: rgba(var(--v-theme-error), 0.05);
  border-color: rgba(var(--v-theme-error), 0.3);
}

.admin-action-card.destructive:hover {
  border-color: rgb(var(--v-theme-error));
  background: rgba(var(--v-theme-error), 0.08);
}

.admin-action-info {
  flex: 1;
}

.admin-action-header {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
}

.admin-action-icon {
  color: rgb(var(--v-theme-error));
  margin-right: 10px;
}

.admin-action-title {
  font-weight: 600;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.admin-action-desc {
  font-size: 14px;
  line-height: 1.5;
  color: rgb(var(--v-theme-on-surface-variant));
  margin: 0;
}

.admin-action-desc code {
  background: rgb(var(--v-theme-surface));
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 13px;
  font-family: 'Roboto Mono', monospace;
  color: rgb(var(--v-theme-primary));
}

/* Responsive Admin */
@media (max-width: 768px) {
  .admin-action-card {
    flex-direction: column;
    align-items: flex-start;
  }

  .admin-action-card .v-btn {
    width: 100%;
  }
}
</style>
