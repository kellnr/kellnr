<template>
  <div class="container" v-if="crate != null">
    <div class="titleSection">
      <span class="k-h1">{{ crate.name }}</span>
      <span class="crateVersion">{{ selected_version.version }}</span>
    </div>
    <p v-if="crate.description != null">
      {{ crate.description }}
    </p>

    <div class="tabSwitch paragraph">
      <div v-if="selected_version.readme" class="tab clickable" :class="tab === 'readme' ? 'activeTab' : ''"
        @click="changeTab('readme')">
        Readme
      </div>
      <div class="tab clickable" :class="tab === 'meta' ? 'activeTab' : ''" @click="changeTab('meta')">
        About
      </div>
      <div class="tab clickable" :class="tab === 'deps' ? 'activeTab' : ''" @click="changeTab('deps')">
        Dependencies
      </div>
      <div class="tab clickable" :class="tab === 'versions' ? 'activeTab' : ''" @click="changeTab('versions')">
        Versions
      </div>
      <div v-if="store.loggedInUserIsAdmin" class="tab clickable" :class="tab === 'crateSettings' ? 'activeTab' : ''"
        @click="changeTab('crateSettings')">
        Settings
      </div>
      <div v-if="store.loggedInUserIsAdmin" class="tab clickable" :class="tab === 'administrate' ? 'activeTab' : ''"
        @click="changeTab('administrate')">
        Admin
      </div>
    </div>

    <div id="infoGrid">
      <div id="tabs" class="">
        <div v-if="tab === 'readme'">
          <Readme :readme="selected_version.readme"></Readme>
        </div>

        <div v-if="tab === 'versions'">
          <div class="">
            <Version v-for="version in crate.versions" :key="version.version" :name="crate.name"
              :version="version.version" :last_updated="version.created" :downloads="version.downloads.toString()" />
          </div>
        </div>

        <div v-if="tab === 'deps'">
          <div class="" v-if="sortedDeps.length > 0">
            <Dependency v-for="dep in sortedDeps" :key="dep.name" :name="dep.name" :version="dep.version_req"
              :registry="dep.registry">
            </Dependency>
          </div>

          <div class="" v-if="sortedDevDeps.length > 0">
            <h2 class="k-h3">Development Dependencies</h2>
            <Dependency v-for="dep in sortedDevDeps" :key="dep.name" :name="dep.name" :version="dep.version_req"
              :registry="dep.registry">
            </Dependency>
          </div>

          <div class="" v-if="sortedDevDeps.length > 0">
            <h2 class="k-h3">Build Dependencies</h2>
            <Dependency v-for="dep in sortedBuildDeps" :key="dep.name" :name="dep.name" :version="dep.version_req"
              :registry="dep.registry" :desc="dep.description">
            </Dependency>
          </div>
        </div>

        <div v-if="tab === 'meta'" class="metaTab">
          <div class="glass">
            <div class="iconElements">
              <IconElement icon="fas fa-link" title="Homepage" v-if="crate.homepage != null">
                <a :href="crate.homepage" class="link" target="_blank">{{
                  crate.homepage
                }}</a>
              </IconElement>
              <IconElement icon="fas fa-balance-scale" title="License" v-if="selected_version.license != null">
                {{ selected_version.license }}
              </IconElement>
              <IconElement icon="fab fa-github" title="Repository" v-if="crate.repository != null">
                <a :href="crate.repository" class="link" target="_blank">{{
                  crate.repository
                }}</a>
              </IconElement>
              <IconElement icon="fas fa-trash-alt" title="Yanked" v-if="selected_version.yanked === true">
                Yes
              </IconElement>
            </div>
            <div class="iconLists">
              <IconList :list="crate.authors" :icon="'fas fa-user'" :title="'Authors'" />
              <IconList :list="crate.categories" :icon="'fas fa-cubes'" :title="'Categories'" />
              <IconList :list="flattenedFeatures" :icon="'fas fa-cog'" :title="'Features'" />
              <IconList :list="crate.keywords" :icon="'fas fa-key'" :title="'Keywords'" />
              <IconList :list="sortedOwners" :icon="'fas fa-user'" :title="'Owners'" />
            </div>
          </div>
        </div>

        <div v-if="tab === 'crateSettings'" class="crateSettingsTab">
          <div class="glass">
            <h2 class="k-h2">Crate access</h2>
            <form>
              <div class="field">
                <label class="checkbox">
                  <input type="checkbox" v-model="is_download_restricted" /> Crate users only are allowed to download
                </label>
              </div>
              <status-notification :status="changeCrateAccessStatus" @update:clear="changeCrateAccessStatus = $event">
                {{ changeCrateAccessMsg }}
              </status-notification>
              <div class="control">
                <button class="button is-info" @click.prevent="setCrateAccessData">Change crate access rules</button>
              </div>
            </form>
          </div>
          <div class="glass">
            <h2 class="k-h2">Crate users</h2>
            <template v-for="user in crateUsers" :key="user.login">
              <div class="glass">
                <span class="userName">{{ user.login }}</span>
                <span class="tag is-danger is-light">
                  <a @click="deleteCrateUser(user.login)">Delete</a>
                </span>
              </div>
            </template>
            <status-notification :status="deleteUserStatus" @update:clear="deleteUserStatus = $event">
              {{ deleteUserMsg }}
            </status-notification>
            <h3 class="k-h3">Add crate user</h3>
            <form>
              <div class="field">
                <div class="control is-expanded has-icons-left">
                  <input class="input is-info" v-model="crateUserName" placeholder="Username" type="text" />
                  <span class="icon is-small is-left">
                    <i class="fas fa-user"></i>
                  </span>
                </div>
              </div>
              <status-notification :status="addCrateUserStatus" @update:clear="addCrateUserStatus = $event">
                {{ addCrateUserMsg }}
              </status-notification>
              <div class="control">
                <button class="button is-info" @click.prevent="addCrateUser">Add</button>
              </div>
            </form>
          </div>
        </div>

        <div v-if="tab === 'administrate'" class="administrateTab">
          <div class="glass">
            <h2 class="k-h2">Delete Crate Version</h2>
            <div class="notification is-light is-danger">
              <strong>Warning:</strong> Deleting a crate version breaks all crates that depend on it!
            </div>
            <div class="paragraph">
              Instead of deleting the crate, think about <a
                href="https://doc.rust-lang.org/cargo/commands/cargo-yank.html" class="link">yanking</a> it instead,
              which
              does not break crates that depend on it.
            </div>
            <br />
            <div>
              <span class="control">
                <button class="button is-danger" @click="deleteVersion(crate.name, selected_version.version)">Delete
                  Version</button>
              </span>
              <span id="deleteCrate" class="control">
                <button class="button is-danger" @click="deleteCrate(crate.name)">Delete Crate</button>
              </span>
            </div>
          </div>
        </div>
      </div>

      <div id="infos" class="glass">
        <crate-sidebar-element icon="fa-code" header="Install" class="bottomBorder">
          <div class="clickable tooltip" @click="copyTomlToClipboard()">
            {{ crate.name }} = "{{ selected_version.version }}"
            <span class="tooltiptext">Copy to clipboard</span>
          </div>
        </crate-sidebar-element>

        <crate-sidebar-element icon="fa-calendar-alt" header="Uploaded" class="bottomBorder">
          <div class="tooltip">
            {{ humanizedLastUpdated }}
            <span class="tooltiptext">{{ crate.last_updated }}</span>
          </div>
        </crate-sidebar-element>

        <crate-sidebar-element icon="fa-book" header="Documentation" class="bottomBorder">
          <div class="docs" @click="openDocsPage()">
            <div v-if="docLink">
              <div class="clickable">{{ crate.name }} ({{ selected_version.version }})</div>
            </div>
            <div v-else>
              <router-link class="clickable" to="/publishdocs">Add</router-link>
            </div>
          </div>
          <div class="buildDocs clickable" v-if="showBuildRustdoc()"
            @click="buildDoc(crate.name, selected_version.version)">
            <span v-if="docLink">
              re-build
            </span>
            <span v-else>
              build
            </span>
          </div>
        </crate-sidebar-element>

        <crate-sidebar-element header="Downloads" icon="fa-cloud-download-alt">
          <div>Version: {{ selected_version.downloads }}</div>
          <div>Total: {{ crate.total_downloads }}</div>
        </crate-sidebar-element>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import Dependency from "../components/Dependency.vue";
import Version from "../components/Version.vue";
import IconList from "../components/IconList.vue";
import IconElement from "../components/IconElement.vue";
import { computed, onBeforeMount, ref, watch } from "vue";
import axios from "axios";
import { useRoute, useRouter } from "vue-router";
import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";
import utc from "dayjs/plugin/utc";
import CrateSidebarElement from "../components/CrateSidebarElement.vue";
import { defaultCrateData, defaultCrateAccessData, defaultCrateVersionData } from "../types/crate_data";
import type { CrateData, CrateAccessData, CrateVersionData, CrateRegistryDep } from "../types/crate_data";
import { CRATE_DATA, CRATE_DELETE_VERSION, CRATE_DELETE_ALL, DOCS_BUILD, CRATE_USERS, CRATE_USER, CRATE_ACCESS_DATA } from "../remote-routes";
import Readme from "../components/Readme.vue";
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

const crate_access = ref<CrateAccessData>(defaultCrateAccessData);
const is_download_restricted = ref(false);
const crateUsers = ref([])
const crateUserName = ref("")
const addCrateUserStatus = ref("")
const addCrateUserMsg = ref("")
const deleteCrateUserStatus = ref("")
const deleteCrateUserMsg = ref("")

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
  const flattened = [];
  for (let key in features) {
    if (features[key].length > 0) {
      flattened.push(key + ": " + features[key].join(", "));
    } else {
      flattened.push(key);
    }
  }
  return flattened.sort();
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

        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
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
  axios
    .get(CRATE_USERS(crate.value.name), { cache: false })
    .then((res) => {
      if (res.status == 200) {
        crateUsers.value = res.data.users;
      }
    })
    .catch((error) => {
      console.log(error);
    });
};

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
    })
    .catch((error) => {
      console.log(error);
    });
}

function getCrateAccessData() {
  axios
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
        changeCrateAccessStatus.value = "Success"; // eslint-disable-line no-undef
        changeCrateAccessMsg.value = "Crate access data successfully changed."; // eslint-disable-line no-undef
        // Update user list
        getCrateAccessData();
      }
    })
    .catch((error) => {
      if (error.response) {
        changeCrateAccessStatus.value = "Error"; // eslint-disable-line no-undef
        changeCrateAccessMsg.value = "Crate access data could not be changed."; // eslint-disable-line no-undef

        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 500) {
          changeCrateAccessMsg.value = "Crate access data could not be changed"; // eslint-disable-line no-undef
        } else {
          changeCrateAccessMsg.value = "Unknown error"; // eslint-disable-line no-undef
        }
      }
    });
}

function copyTomlToClipboard() {
  const text =
    crate.value.name + ' = "' + selected_version.value.version + '"';
  navigator.clipboard.writeText(text);
}

function openDocsPage() {
  if (selected_version.value.documentation) {
    let url = selected_version.value.documentation;
    window.open(url, "_blank");
  } else {
    router.push({ name: "PublishDocs" })
  }
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
  changeTab(defaultTab.value)
})
</script>

<style scoped>
.container {
  width: 100%;
}

.titleSection {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  width: 100%;
}

.k-h1 {
  margin: 0 1rem 0 0;
  word-wrap: break-word;
  max-width: 100%;
}

.crateVersion {
  font-size: x-large;
}

.tabSwitch {
  width: fit-content;
  max-width: 100%;
  display: flex;
  flex-wrap: wrap;
  margin: 1rem 0 1rem 0;
  border-radius: 2rem;
  padding-left: 1rem;
  padding-right: 1rem;
  background: rgba(248, 248, 248, 0.7);
  box-shadow: 0 2px 2px rgba(0, 0, 0, 0.1);
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
}

body[color-theme="dark"] .tabSwitch {
  background-color: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.06);
}

.tab {
  flex-grow: 1;
  padding: 0.5rem;
  font-weight: bolder;
  text-align: center;
  color: var(--color-darkest);
}

body[color-theme="light"] .activeTab {
  color: var(--color-dark);
}

body[color-theme="dark"] .activeTab {
  color: var(--dark-color-middle) !important;
}

#deleteCrate {
  margin-left: 1rem;
}

#crateVersion {
  font-size: x-large;
  margin-left: 1rem;
}

#infoGrid {
  display: grid;
  column-gap: 2rem;
  margin-bottom: 1rem;
}

#tabs {
  grid-area: tabs;
}

#infos {
  margin: 0 0 0 0rem;
  height: fit-content;
}

.bottomBorder {
  border-bottom: 0.05rem;
  border-bottom-style: solid;
}

.buildDocs {
  font-size: smaller;
}

@media only screen and (max-width: 768px) {
  #infoGrid {
    grid-template-rows: auto auto;
    grid-template-areas:
      "infos"
      "tabs";
    width: 100%;
  }
}

@media only screen and (min-width: 768px) {
  #infoGrid {
    grid-template-columns: 3fr 1fr;
    grid-template-areas: "tabs infos";
  }
}

@media only screen and (min-width: 992px) {}
</style>
