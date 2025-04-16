<template>
  <v-card class="mb-2" rounded="lg" hover @click="openCratePage">
    <v-row no-gutters>
      <!-- Left Column with Name, Version, and Source -->
      <v-col cols="12" sm="3" class="border-r">
        <v-card-item>
          <!-- Name and Version on same line -->
          <div class="d-flex align-center flex-wrap">
            <h3 class="text-body-1 font-weight-bold me-2">{{ name }}</h3>
            <span class="text-primary font-weight-medium">{{ version }}</span>
          </div>

          <!-- Source Indicator below name/version -->
          <div class="mt-2">
            <v-chip size="small" variant="flat" :color="isCratesIoDep(registry) ? 'warning' : 'primary'"
              :prepend-icon="isCratesIoDep(registry) ? 'mdi-package-variant' : 'mdi-package-variant-closed'"
              density="comfortable">
              {{ isCratesIoDep(registry) ? 'crates.io' : 'kellnr' }}
            </v-chip>
          </div>
        </v-card-item>
      </v-col>

      <!-- Description Column -->
      <v-col cols="12" sm="9">
        <v-card-item>
          <v-card-text class="text-body-2">
            <div v-if="fetched_desc">{{ fetched_desc }}</div>
            <div v-else class="text-italic">
              <v-progress-circular indeterminate size="16" width="2" color="primary" class="me-2"></v-progress-circular>
              Loading description...
            </div>
          </v-card-text>
        </v-card-item>
      </v-col>
    </v-row>
  </v-card>
</template>

<script setup lang="ts">
import { onBeforeMount, ref, watch } from "vue";
import axios from "axios";
import { useStore } from "../store/store"
import { useRoute, useRouter } from "vue-router";
import type { CrateData } from "../types/crate_data";
import { CRATE_DATA, CRATESIO_DATA, CRATESIO_LINK } from "../remote-routes";

const props = defineProps<{
  name: string
  version: string
  registry?: string
  desc?: string
}>()

const fetched_desc = ref("")
const route = useRoute()
const router = useRouter()
const store = useStore()

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
      params: { name: crate },
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
      params: { name: crate },
    })
    .then((response) => {
      let crateData: CrateData = response.data;
      if (crateData) {
        fetched_desc.value = crateData.description == null ? "No description set" : crateData.description;
      } else {
        fetched_desc.value = "Cannot fetch description.";
      }
    })
    .catch(() => {
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
  router.push({ name: 'Crate', query: { name: props.name } })
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
watch(route, () => {
  setDesc(props.name, props.version, props.registry)
})
</script>

<style scoped>
.border-r {
  border-right: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

@media (max-width: 600px) {
  .border-r {
    border-right: none;
    border-bottom: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  }
}
</style>
