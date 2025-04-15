<template>
  <v-container fluid class="pa-0 d-flex flex-column">
    <!-- Search Header -->
    <v-card class="pa-3 ma-3" elevation="2" rounded="lg" color="surface">
      <v-row no-gutters align="center">
        <v-col cols="12" md="8" lg="6" class="pr-md-4">
          <v-text-field v-model="searchText" placeholder="Search for crates" variant="outlined" density="comfortable"
            hide-details prepend-inner-icon="mdi-magnify" color="primary" bg-color="surface"
            @keyup.enter="searchCrates(searchText)" class="search-field" rounded="lg"></v-text-field>
        </v-col>

        <v-col cols="12" md="4" lg="6" class="mt-3 mt-md-0 d-flex align-center">
          <v-switch v-model="store.searchCache" color="primary" hide-details @update:model-value="refreshCrates()">
            <template v-slot:label>
              <div class="d-flex align-center">
                <span class="mr-2">Crates proxy</span>
                <v-tooltip location="top" text="Display crates from the crates.io proxy">
                  <template v-slot:activator="{ props }">
                    <v-icon icon="mdi-information-outline" size="small" v-bind="props" />
                  </template>
                </v-tooltip>
              </div>
            </template>
          </v-switch>
        </v-col>
      </v-row>
    </v-card>

    <!-- Crates List with Infinite Scroll -->
    <v-card class="flex-grow-1 overflow-auto" flat color="transparent" rounded="lg" ref="cratesContainer"
      @scroll="handleScroll">
      <!-- Empty State -->
      <v-card v-if="crates.length === 0 && !isLoading" class="pa-6 text-center mx-auto my-8" max-width="500"
        variant="outlined">
        <v-card-title class="text-h6 font-weight-medium">No crates found</v-card-title>
        <v-card-text>
          <p>To learn how to publish crates to <strong>Kellnr</strong>, read the
            <a href="https://kellnr.io/documentation" target="_blank" class="text-decoration-none font-weight-medium">
              documentation
            </a>.
          </p>
          <v-icon icon="mdi-package-variant" size="x-large" color="grey-lighten-1" class="my-4"></v-icon>
        </v-card-text>
      </v-card>

      <!-- Crates Grid -->
      <v-row class="pa-3">
        <v-col cols="12">
          <crate-card v-for="crate in crates" :key="`${crate.name}-${crate.version}`" :crate="crate.name"
            :version="crate.version" :updated="crate.date" :downloads="crate.total_downloads" :desc="crate.description"
            :doc-link="crate.documentation" :is-cache="crate.is_cache"></crate-card>
        </v-col>
      </v-row>

      <!-- Loading Indicator -->
      <div v-if="isLoading" class="text-center my-4 pb-4">
        <v-progress-circular indeterminate color="primary" :size="40"></v-progress-circular>
        <div class="text-body-2 mt-2">Loading crates...</div>
      </div>

      <!-- End of Results -->
      <div v-if="allLoaded && crates.length > 0" class="text-center my-6 text-body-2 text-grey">
        — End of crates —
      </div>
    </v-card>
  </v-container>
</template>

<script setup lang="ts">
import { onBeforeMount, onMounted, ref } from "vue"
import axios from "axios"
import CrateCard from "../components/CrateCard.vue"
import type { CrateOverview } from "../types/crate_overview";
import { CRATES, SEARCH } from "../remote-routes";
import { useRouter } from "vue-router";
import { useStore } from "../store/store";

// Constants
const ITEMS_PER_PAGE = 20;

// State
const crates = ref<Array<CrateOverview>>([]);
const currentPage = ref(0);
const isLoading = ref(false);
const allLoaded = ref(false);
const searchText = ref("");
const cratesContainer = ref<HTMLElement | null>(null);
const router = useRouter();
const store = useStore();

// Initial setup
onBeforeMount(() => {
  if (router.currentRoute.value.query.search) {
    searchText.value = router.currentRoute.value.query.search as string;
    searchCrates(searchText.value);
  }
});

onMounted(() => {
  if (searchText.value === "") {
    loadMoreCrates();
  }
});

// Load more crates for infinite scrolling
function loadMoreCrates() {
  if (isLoading.value || allLoaded.value) return;

  isLoading.value = true;

  axios
    .get(CRATES, {
      params: {
        page: currentPage.value,
        page_size: ITEMS_PER_PAGE,
        cache: store.searchCache
      },
    })
    .then((response) => {
      const newCrates = response.data.crates;

      // Add crates to the list
      crates.value = [...crates.value, ...newCrates];

      // Increment page for next load
      currentPage.value = response.data.page + 1;

      // Check if we've loaded all available crates
      if (newCrates.length < ITEMS_PER_PAGE) {
        allLoaded.value = true;
      }
    })
    .catch((error) => {
      console.error("Error loading crates:", error);
    })
    .finally(() => {
      isLoading.value = false;
    });
}

// Handle scroll event for infinite scrolling
function handleScroll() {
  if (searchText.value !== "") return;

  const container = cratesContainer.value;
  if (!container) return;

  // Calculate if we're near the bottom (within 200px)
  const bottomPosition = container.scrollHeight - container.clientHeight;
  const isNearBottom = container.scrollTop > bottomPosition - 200;

  if (isNearBottom && !isLoading.value && !allLoaded.value) {
    loadMoreCrates();
  }
}

// Refresh crates (used when changing filters)
function refreshCrates() {
  crates.value = [];
  currentPage.value = 0;
  allLoaded.value = false;

  loadMoreCrates();
}

// Search crates by name
function searchCrates(searchText: string) {
  const searchQuery = searchText.trim();

  if (!searchQuery) {
    refreshCrates();
    return;
  }

  isLoading.value = true;
  allLoaded.value = false;

  axios
    .get(SEARCH, {
      params: {
        name: searchQuery,
        cache: store.searchCache
      }
    })
    .then((res) => {
      crates.value = res.data.crates;
      allLoaded.value = true; // Search results are all loaded at once
    })
    .catch(() => {
      crates.value = [];
      allLoaded.value = true;
    })
    .finally(() => {
      isLoading.value = false;
    });
}
</script>

<style scoped>
.search-field :deep(.v-field__input) {
  padding-top: 10px;
  padding-bottom: 10px;
  min-height: 44px;
}

.fill-height {
  height: 100%;
}

/* Improve scrollbar appearance */
:deep(.v-card.overflow-auto::-webkit-scrollbar) {
  width: 6px;
}

:deep(.v-card.overflow-auto::-webkit-scrollbar-thumb) {
  background-color: rgba(0, 0, 0, 0.2);
  border-radius: 3px;
}

:deep(.v-card.overflow-auto::-webkit-scrollbar-track) {
  background: transparent;
}

/* Dark mode adjustments */
:deep(.v-theme--dark .v-card.overflow-auto::-webkit-scrollbar-thumb) {
  background-color: rgba(255, 255, 255, 0.2);
}
</style>
