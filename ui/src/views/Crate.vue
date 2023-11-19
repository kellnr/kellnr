<template>
  <div v-if="crate != null">
    <div class="">
      <div>
        <span id="crateTitle" class="k-h1">{{ crate.name }}</span>
        <span id="crateVersion">{{ selected_version.version }}</span>
      </div>

      <div class="paragraph" v-if="crate.description != null">
        <p id="crateDesc">
          {{ crate.description }}
        </p>
      </div>
    </div>

    <div class="tabSwitch paragraph">
      <div v-if="selected_version.readme"
        class="tab clickable"
        :class="tab === 'readme' ? 'activeTab' : ''"
        @click="changeTab('readme')"
      >
        Readme
      </div>
      <div
          class="tab clickable"
          :class="tab === 'meta' ? 'activeTab' : ''"
          @click="changeTab('meta')"
      >
        About
      </div>
      <div
          class="tab clickable"
          :class="tab === 'deps' ? 'activeTab' : ''"
          @click="changeTab('deps')"
      >
        Dependencies
      </div>
      <div
          class="tab clickable"
          :class="tab === 'versions' ? 'activeTab' : ''"
          @click="changeTab('versions')"
      >
        Versions
      </div>
      <div
          v-if="store.state.loggedInUserIsAdmin"
          class="tab clickable"
          :class="tab === 'administrate' ? 'activeTab' : ''"
          @click="changeTab('administrate')"
      >
        Admin
      </div>
    </div>

    <div id="infoGrid">
      <div id="tabs" class="">
        <div v-if="tab === 'readme'">
          <Readme
              :readme="selected_version.readme"
          ></Readme>
        </div>

        <div v-if="tab === 'versions'">
          <div class="">
            <Version
                v-for="version in crate.versions"
                :key="version.version"
                :name="crate.name"
                :version="version.version"
                :last_updated="version.created"
                :downloads="version.downloads.toString()"
            />
          </div>
        </div>

        <div v-if="tab === 'deps'">
          <div class="" v-if="sortedDeps.length > 0">
            <Dependency
                v-for="dep in sortedDeps"
                :key="dep.name"
                :name="dep.name"
                :version="dep.version_req"
                :registry="dep.registry"
            >
            </Dependency>
          </div>

          <div class="" v-if="sortedDevDeps.length > 0">
            <h2 class="k-h3">Development Dependencies</h2>
            <Dependency
                v-for="dep in sortedDevDeps"
                :key="dep.name"
                :name="dep.name"
                :version="dep.version_req"
                :registry="dep.registry"
            >
            </Dependency>
          </div>

          <div class="" v-if="sortedDevDeps.length > 0">
            <h2 class="k-h3">Build Dependencies</h2>
            <Dependency
                v-for="dep in sortedBuildDeps"
                :key="dep.name"
                :name="dep.name"
                :version="dep.version_req"
                :registry="dep.registry"
                :desc="dep.description"
            >
            </Dependency>
          </div>
        </div>

        <div v-if="tab === 'meta'" class="metaTab">
          <div class="glass">
            <div class="iconElements">
              <IconElement
                  icon="fas fa-link"
                  title="Homepage"
                  v-if="crate.homepage != null"
              >
                <a :href="crate.homepage" class="link" target="_blank">{{
                    crate.homepage
                  }}</a>
              </IconElement>
              <IconElement
                  icon="fas fa-balance-scale"
                  title="License"
                  v-if="selected_version.license != null"
              >
                {{ selected_version.license }}
              </IconElement>
              <IconElement
                  icon="fab fa-github"
                  title="Repository"
                  v-if="crate.repository != null"
              >
                <a :href="crate.repository" class="link" target="_blank">{{
                    crate.repository
                  }}</a>
              </IconElement>
              <IconElement
                  icon="fas fa-trash-alt"
                  title="Yanked"
                  v-if="selected_version.yanked === true"
              >
                Yes
              </IconElement>
            </div>
            <div class="iconLists">
              <IconList
                  :list="crate.authors"
                  :icon="'fas fa-user'"
                  :title="'Authors'"
              />
              <IconList
                  :list="crate.categories"
                  :icon="'fas fa-cubes'"
                  :title="'Categories'"
              />
              <IconList
                  :list="flattenedFeatures"
                  :icon="'fas fa-cog'"
                  :title="'Features'"
              />
              <IconList
                  :list="crate.keywords"
                  :icon="'fas fa-key'"
                  :title="'Keywords'"
              />
              <IconList
                  :list="sortedOwners"
                  :icon="'fas fa-user'"
                  :title="'Owners'"
              />
            </div>
          </div>
        </div>

        <div v-if="tab === 'administrate'" class="administrateTab">
          <div class="glass">
            <h2 class="k-h2">Delete Crate Version</h2>
            <div class="notification is-light is-danger">
              <strong>Warning:</strong> Deleting a crate version breaks all crates that depend on it!
            </div>
            <div class="paragraph">
              Instead of deleting the crate, think about <a href="https://doc.rust-lang.org/cargo/commands/cargo-yank.html" class="link">yanking</a> it instead, which does not break crates that depend on it.
            </div>
            <br/>
            <div class="control">
              <button class="button is-danger" @click="deleteVersion(crate.name, selected_version.version)">Delete</button>
            </div>
          </div>
        </div>
      </div>

      <div id="infos" class="glass">
        <crate-sidebar-element icon="fa-code" header="Install" class="bottomBorder">
          <div
              class="clickable tooltip"
              @click="copyTomlToClipboard()"
          >
            {{ crate.name }} = "{{ selected_version.version }}"
            <span class="tooltiptext">Copy to clipboard</span>
          </div>
        </crate-sidebar-element>

        <crate-sidebar-element icon="fa-calendar-alt" header="Uploaded"  class="bottomBorder">
          <div class="tooltip">
            {{ humanizedLastUpdated }}
            <span class="tooltiptext">{{ crate.last_updated }}</span>
          </div>
        </crate-sidebar-element>

        <crate-sidebar-element icon="fa-book" header="Documentation" class="bottomBorder">
          <div class="docs" @click="openDocsPage()">
            <div v-if="docLink">
              <div class="clickable">{{ crate.name }} ({{selected_version.version}})</div>
            </div>
            <div v-else>
              <router-link class="clickable" to="/publishdocs">Add</router-link>
            </div>
          </div>
          <div class="buildDocs clickable" v-if="showBuildRustdoc()" @click="buildDoc(crate.name, selected_version.version)">
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
import {computed, onBeforeMount, ref, watch} from "vue";
import axios from "axios";
import {useRoute, useRouter} from "vue-router";
import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";
import CrateSidebarElement from "../components/CrateSidebarElement.vue";
import {store} from "../store/store";
import {CrateData, CrateVersionData, defaultCrateData, defaultCrateVersionData, CrateRegistryDep} from "../types/crate_data";
import {CRATE_DATA, CRATE_DELETE, DOCS_BUILD, kellnr_url} from "../remote-routes";
import Readme from "../components/Readme.vue";

dayjs.extend(relativeTime);

const crate = ref<CrateData>(defaultCrateData);
const router = useRouter()
const route = useRoute()
const selected_version = ref<CrateVersionData>(defaultCrateVersionData)
const defaultTab = ref<string>("meta")
const tab = ref(defaultTab);

const docLink = computed(() => {
  return selected_version.value.documentation;
})

const humanizedLastUpdated = computed(() => {
  return dayjs(crate.value.last_updated).fromNow();
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

function deleteVersion(crate: string, version: string) {
  if(confirm('Delete "' + crate + '" version "' + version + '"?')) {
    axios.delete(CRATE_DELETE,
      {
        params: {
          name: crate,
          version: version
        }
      }
    ).then((_response) => {
      router.push({name: "Crates"})
    }).catch((error) => {
      console.log(error);
    });
  }
}

function showBuildRustdoc() : boolean {
  // Show the option to build the docs, if the current logged-in user is and admin
  if(store.state.loggedInUserIsAdmin) {
    return true
  }

  // Show the option to build the docs, if the current logged-in user owns the crate
  return crate.value.owners.includes(store.state.loggedInUser);
}

function buildDoc(crate: string, version: string) {
  axios.post(DOCS_BUILD, null, { params: { package: crate, version: version}})
      .then((_res) => {
        router.push({name: "DocQueue"})
      })
      .catch((error) => {
        console.log(error)
      })
}

function changeTab(newTab: string) {
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
      .get(CRATE_DATA, {params: {name: name}})
      .then((response) => {
        crate.value = response.data;
        version = version ?? crate.value.max_version;
        selected_version.value = crate.value.versions.find((cvd: CrateVersionData) => {
          return cvd.version ==  version;
        }) ?? defaultCrateVersionData;

        // Set the default tab to "readme" if a readme is available, else "meta"
        defaultTab.value = selected_version.value.readme == null ? "meta" : "readme";
      })
      .catch((error) => {
        console.log(error);
      });
}

function copyTomlToClipboard() {
  const text =
      crate.value.name + ' = "' + selected_version.value.version + '"';
  navigator.clipboard.writeText(text);
}

function openDocsPage() {
  if (selected_version.value.documentation) {
    let url = selected_version.value.documentation.startsWith("/docs") ?
        kellnr_url(selected_version.value.documentation) : selected_version.value.documentation;
    window.open(url, "_blank");
  } else {
    router.push({name: "PublishDocs"})
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
watch(route, (_oldRoute, _newRoute) => {
  getAllData()
  changeTab(defaultTab.value)
})
</script>

<style>
.paragraph {
  margin: 2rem 0 0 0;
}

/*body[color-theme="light"] .border-element {*/
/*  border-color: var(--dark-color);*/
/*}*/

.tabSwitch {
  display: grid;
  grid-template-columns: max-content max-content max-content max-content max-content;
  margin-bottom: 1rem;
  border-radius: 2rem;
  padding-left: 1rem;
  padding-right: 1rem;
  width: fit-content;

  background: rgba(248,248,248, 0.7);
  box-shadow: 0 2px 2px rgba(0, 0, 0, 0.1);
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
}

body[color-theme="dark"] .tabSwitch {
  background-color: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.06);
}

.tab {
  padding: 0.5rem;
  font-weight: bolder;
  color: var(--color-darkest);
}

body[color-theme="light"] .activeTab {
  color: var(--color-dark);
}

body[color-theme="dark"] .activeTab {
  color: var(--dark-color-middle) !important;
}

#crateVersion {
  font-size: x-large;
  margin-left: 1rem;
}

#infoGrid {
  display: grid;
  grid-template-columns: 3fr 1fr;
}

#infos {
  margin: 0 0 0 2rem;
  height: fit-content;
}

.buildDocs {
  font-size: smaller;
}

.bottomBorder {
  border-bottom: 0.05rem;
  border-bottom-style: solid;
}
</style>
