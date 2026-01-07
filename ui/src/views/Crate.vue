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
        <!-- Readme Tab -->
        <v-card v-if="tab === 'readme'" class="mb-4" elevation="1">
          <Readme :readme="selected_version.readme"></Readme>
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
          <About :crate="crate" :selected-version="selected_version" :flattened-features="flattenedFeatures"
            :sorted-owners="sortedOwners" />
        </v-card>

        <!-- Settings Tab -->
        <template v-if="tab === 'crateSettings'">
          <!-- Crate Access -->
          <v-card class="mb-4" elevation="1">
            <v-card-title>Crate access</v-card-title>
            <v-card-text>
              <v-form @submit.prevent="setCrateAccessData">
                <v-checkbox v-model="is_download_restricted"
                  label="Crate users only are allowed to download"></v-checkbox>

                <v-alert v-if="changeCrateAccessStatus"
                  :type="changeCrateAccessStatus === 'Success' ? 'success' : 'error'" closable
                  @click:close="changeCrateAccessStatus = ''">
                  {{ changeCrateAccessMsg }}
                </v-alert>

                <v-btn color="primary" type="submit">
                  Change crate access rules
                </v-btn>
              </v-form>
            </v-card-text>
          </v-card>

          <!-- Crate Users -->
          <v-card class="mb-4" elevation="1">
            <v-card-title>Crate users</v-card-title>
            <v-card-text>
              <v-list>
                <v-list-item v-for="user in crateUsers" :key="user.login">
                  <v-list-item-title>{{ user.login }}</v-list-item-title>
                  <template v-slot:append>
                    <v-btn color="error" variant="text" size="small" @click="deleteCrateUser(user.login)">
                      Delete
                    </v-btn>
                  </template>
                </v-list-item>
              </v-list>

              <v-alert v-if="deleteCrateUserStatus" :type="deleteCrateUserStatus === 'Success' ? 'success' : 'error'"
                closable @click:close="deleteCrateUserStatus = ''" class="mt-4">
                {{ deleteCrateUserMsg }}
              </v-alert>

              <v-divider class="my-4"></v-divider>

              <h3 class="text-h5 mb-3">Add crate user</h3>
              <v-form @submit.prevent="addCrateUser">
                <v-text-field v-model="crateUserName" placeholder="Username" prepend-icon="mdi-account"
                  variant="outlined" density="comfortable"></v-text-field>

                <v-alert v-if="addCrateUserStatus" :type="addCrateUserStatus === 'Success' ? 'success' : 'error'"
                  closable @click:close="addCrateUserStatus = ''" class="my-2">
                  {{ addCrateUserMsg }}
                </v-alert>

                <v-btn color="primary" type="submit">
                  Add
                </v-btn>
              </v-form>
            </v-card-text>
          </v-card>

          <!-- Crate Groups -->
          <v-card class="mb-4" elevation="1">
            <v-card-title>Crate groups</v-card-title>
            <v-card-text>
              <v-list>
                <v-list-item v-for="group in crateGroupsForCrate" :key="group.name">
                  <v-list-item-title>{{ group.name }}</v-list-item-title>
                  <template v-slot:append>
                    <v-btn color="error" variant="text" size="small" @click="deleteCrateGroup(group.name)">
                      Delete
                    </v-btn>
                  </template>
                </v-list-item>
              </v-list>

              <v-alert v-if="deleteCrateGroupStatus" :type="deleteCrateGroupStatus === 'Success' ? 'success' : 'error'"
                closable @click:close="deleteCrateGroupStatus = ''" class="mt-4">
                {{ deleteCrateGroupMsg }}
              </v-alert>

              <v-divider class="my-4"></v-divider>

              <h3 class="text-h5 mb-3">Add crate group</h3>
                <v-form @submit.prevent="addCrateGroup">
                  <v-select v-model="crateGroupName" :items="availableCrateGroups" label="Select Group" prepend-icon="mdi-account-group" variant="outlined" density="comfortable" class="mb-2">
                  </v-select>

                <v-alert v-if="addCrateGroupStatus" :type="addCrateGroupStatus === 'Success' ? 'success' : 'error'"
                  closable @click:close="addCrateGroupStatus = ''" class="my-2">
                  {{ addCrateGroupMsg }}
                </v-alert>

                <v-btn color="primary" type="submit">
                  Add
                </v-btn>
              </v-form>
            </v-card-text>
          </v-card>
        </template>

        <!-- Admin Tab -->
        <v-card v-if="tab === 'administrate'" class="mb-4" elevation="1">
          <v-card-title>Delete Crate Version</v-card-title>
          <v-card-text>
            <v-alert type="error" variant="tonal" class="mb-4">
              <strong>Warning:</strong> Deleting a crate version breaks all crates that depend on it!
            </v-alert>

            <p class="mb-4">
              Instead of deleting the crate, think about
              <a href="https://doc.rust-lang.org/cargo/commands/cargo-yank.html" class="text-primary">yanking</a> it
              instead,
              which does not break crates that depend on it.
            </p>

            <v-row>
              <v-col>
                <v-btn color="error" @click="deleteVersion(crate.name, selected_version.version)">
                  Delete Version
                </v-btn>
              </v-col>
              <v-col>
                <v-btn color="error" @click="deleteCrate(crate.name)">
                  Delete Crate
                </v-btn>
              </v-col>
            </v-row>
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
import type { CrateData, CrateAccessData, CrateVersionData, CrateRegistryDep } from "../types/crate_data";
import { CRATE_DATA, CRATE_DELETE_VERSION, CRATE_DELETE_ALL, DOCS_BUILD, CRATE_USERS, CRATE_USER, CRATE_GROUPS, CRATE_GROUP, CRATE_ACCESS_DATA, LIST_GROUPS } from "../remote-routes";
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
  const crateGroupsForCrate = ref([])
  const crateGroups = ref<string[]>([])
  const crateGroupName = ref("")

  const availableCrateGroups = computed(() => {
    const assigned = new Set(
      (crateGroupsForCrate.value as Array<{ name?: unknown }>).
        map((g) => (typeof g?.name === "string" ? g.name : ""))
        .filter((n) => n.length > 0)
    )

    return crateGroups.value.filter((name) => !assigned.has(name))
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
    .put(CRATE_USER(crate.value.name, crateUserName.value))
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
      .delete(CRATE_USER(crate.value.name, name))
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
    .get(CRATE_USERS(crate.value.name), { cache: false })
    .then((res) => {
      if (res.status == 200) {
        crateUsers.value = res.data.users;
      }
    })
    .catch((error) => {
      console.log(error);
    });
}

function addCrateGroup() {
  axios
    .put(CRATE_GROUP(crate.value.name, crateGroupName.value))
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
      .delete(CRATE_GROUP(crate.value.name, name))
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
  // disable caching to get updated token list
  axios
    // @ts-expect-error TS doesn't recognize cache option
    .get(CRATE_GROUPS(crate.value.name), { cache: false })
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
    // disable caching to get updated token list
    try {
      // @ts-expect-error TS doesn't recognize cache option
      const res = await axios.get(LIST_GROUPS, { cache: false })
      if (res.status == 200) {
        // LIST_GROUPS returns: [{"name":"group1"},{"name":"group2"}]
        crateGroups.value = (res.data ?? [])
          .map((g: { name?: unknown }) => (typeof g?.name === "string" ? g.name : ""))
          .filter((name: string) => name.length > 0)
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
    .get(CRATE_ACCESS_DATA(crate.value.name), { cache: false })
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
    .put(CRATE_ACCESS_DATA(crate.value.name), putData)
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
      getCrateGroupsForCrate();
      getAllCrateGroups();
    }
  })
</script>

<style>
.text-break {
  word-break: break-word;
}
</style>
