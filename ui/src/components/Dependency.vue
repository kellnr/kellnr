<template>
  <div class="dependency clickable glass" @click="openCratePage">
    <div class="depNameLogo">
      <div class="depName">{{ name }}</div>
      <div class="depLogo">
        <img
            v-if="isCratesIoDep(registry)"
            v-bind:src="store.state.cargoSmallLogo"
            class="degLogoImg"
            alt="Crates.io logo"
        />
        <img
            v-else
            v-bind:src="store.state.kellnrSmallLogo"
            class="degLogoImg"
            alt="Kellnr logo"
        />
      </div>
    </div>
    <div class="depVerDesc">
      <div class="depVer">{{ version }}</div>
      <div>{{ fetched_desc }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {onBeforeMount, ref, watch} from "vue";
import axios from "axios";
import {store} from "../store/store"
import {useRoute, useRouter} from "vue-router";
import {CrateData} from "../types/crate_data";
import {CRATE_DATA, CRATESIO_DATA, CRATESIO_LINK} from "../remote-routes";

const props = defineProps<{
  name: string
  version: string
  registry?: string
  desc?: string
}>()

const fetched_desc = ref("")
const route = useRoute()
const router = useRouter()

onBeforeMount(() => {
  if (!props.desc) {
    setDesc(props.name, props.version, props.registry);
  } else {
    fetched_desc.value = props.desc
  }
})

function getCratesIoUrl(crate: string) {
  return CRATESIO_LINK(crate);
}

function isCratesIoDep(registry: string | undefined) {
  return registry === "https://github.com/rust-lang/crates.io-index";
}

function setDescFromCratesIo(crate: string) {
  axios
      .get(CRATESIO_DATA, {
        params: {name: crate},
      })
      .then((response) => {
        fetched_desc.value = response.data.crate.description;
      })
      .catch((error) => {
        console.log(error);
        fetched_desc.value = "Cannot fetch description.";
      });
}

function setDescFromKellnr(crate: string) {
  axios
      .get(CRATE_DATA, {
        params: {name: crate},
      })
      .then((response) => {
        let crateData: CrateData = response.data;
        if (crateData) {
          fetched_desc.value = crateData.description == null ? "No description set" : crateData.description;
        } else {
          fetched_desc.value = "Cannot fetch description.";
        }
      })
      .catch((_error) => {
        fetched_desc.value = "Cannot fetch description.";
      });
}

function setDesc(crate: string, version: string, registry: string | undefined) {
  if (isCratesIoDep(registry)) {
    setDescFromCratesIo(crate);
  } else {
    setDescFromKellnr(crate);
  }
}

function openCratesIoPage() {
  window.open(getCratesIoUrl(props.name));
}

function openKellnrPage() {
  router.push({name: 'Crate', query: {name: props.name}})
}

function openCratePage() {
  if (isCratesIoDep(props.registry)) {
    openCratesIoPage();
  } else {
    openKellnrPage();
  }
}

// Watches route changes and reloads the data.
// Needed, if the query parameter "name=crate" changes.
watch(route, (_oldRoute, _newRoute) => {
  setDesc(props.name, props.version, props.registry)
})

</script>

<style>
.dependency {
  padding: 0.5rem;
  margin: 0 0 0.5rem 0;
  display: grid;
  grid-template-columns: minmax(8rem, max-content) auto;
}

.depNameLogo {
  display: grid;
  grid-template-rows: max-content max-content;
  padding: 0 0.5rem 0 0;
  border-right-style: solid;
  border-width: 0.1rem;
}

.depName {
  font-weight: bold;
}

.depVer {
  font-weight: bold;
}

.depVerDesc {
  display: grid;
  grid-template-rows: auto auto;
  padding: 0 0 0 0.5rem;
}

.depName {
  text-align: center;
}

.depLogo {
  text-align: center;
}

.degLogoImg {
  max-width: 2rem;
}
</style>
