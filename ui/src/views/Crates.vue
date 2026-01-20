<template>
  <v-container fluid class="pa-0 main-container">
    <!-- Search Header -->
    <v-card class="search-card pa-4 ma-3" elevation="0" rounded="lg">
      <v-row no-gutters align="center">
        <v-col cols="12" md="8" lg="6" class="pr-md-4">
          <v-text-field v-model="searchText" placeholder="Search for crates" variant="outlined" density="comfortable"
            hide-details prepend-inner-icon="mdi-magnify" color="primary"
            @keyup.enter="handleSearch(searchText)" class="search-field" rounded="lg"></v-text-field>
        </v-col>

        <v-col cols="12" md="4" lg="6" class="mt-3 mt-md-0 d-flex align-center">
          <v-switch v-model="store.searchCache" color="primary" hide-details @update:model-value="refreshCrates()">
            <template v-slot:label>
              <div class="d-flex align-center">
                <span class="mr-2 switch-label">Crates proxy</span>
                <v-tooltip location="top" text="Display crates from the crates.io proxy">
                  <template v-slot:activator="{ props }">
                    <v-icon icon="mdi-information-outline" size="small" v-bind="props" class="info-icon" />
                  </template>
                </v-tooltip>
              </div>
            </template>
          </v-switch>
        </v-col>
      </v-row>
    </v-card>

    <!-- Scrollable Content Container -->
    <div class="content-container" ref="scrollContainer" @scroll="handleScroll">
      <!-- Empty State -->
      <v-card v-if="crates.length === 0 && !isLoading" class="pa-6 text-center mx-auto my-8" max-width="500"
        variant="outlined">
        <v-card-title class="text-h6 font-weight-medium">No crates found</v-card-title>
        <v-card-text>
          <p>To learn how to publish crates to <strong>Kellnr</strong>, read the
            <a href="https://kellnr.io/documentation" target="_blank" class="text-decoration-none font-weight-medium">
              documentation
            </a>
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
    </div>
  </v-container>
</template>

<script setup lang="ts">
import { onBeforeMount, onMounted, ref, nextTick } from "vue"
import CrateCard from "../components/CrateCard.vue"
import type { CrateOverview } from "../types/crate_overview"
import { crateService } from "../services"
import { isSuccess } from "../services/api"
import { useRouter } from "vue-router"
import { useStore } from "../store/store"

// Constants
const ITEMS_PER_PAGE = 20

// State
const crates = ref<CrateOverview[]>([])
const currentPage = ref(0)
const isLoading = ref(false)
const allLoaded = ref(false)
const searchText = ref("")
const scrollContainer = ref<HTMLElement | null>(null)
const router = useRouter()
const store = useStore()

// Initial setup
onBeforeMount(() => {
  if (router.currentRoute.value.query.search) {
    searchText.value = router.currentRoute.value.query.search as string
    handleSearch(searchText.value)
  }
})

onMounted(() => {
  if (searchText.value === "") {
    loadMoreCrates()
  }

  // Add resize event listener to handle window size changes
  window.addEventListener('resize', updateContainerHeight)
  // Initial height setup
  updateContainerHeight()
})

// Update container height to fill available space
function updateContainerHeight() {
  nextTick(() => {
    if (scrollContainer.value) {
      const headerHeight = document.querySelector('.v-card.pa-3.ma-3')?.clientHeight || 0
      const headerMargin = 24 // 3 * 8px (ma-3)

      // Calculate and set the height of the scrollable container
      const windowHeight = window.innerHeight
      const footerHeight = 48 // Height of the footer if present
      const availableHeight = windowHeight - headerHeight - headerMargin - footerHeight - 16

      scrollContainer.value.style.height = `${Math.max(300, availableHeight)}px`
    }
  })
}

// Load more crates for infinite scrolling
async function loadMoreCrates() {
  if (isLoading.value || allLoaded.value) return

  isLoading.value = true

  const result = await crateService.getCrates(
    currentPage.value,
    ITEMS_PER_PAGE,
    store.searchCache
  )

  isLoading.value = false

  if (isSuccess(result)) {
    const newCrates = result.data.crates

    // Add crates to the list
    crates.value = [...crates.value, ...newCrates]

    // Increment page for next load
    currentPage.value = result.data.page + 1

    // Check if we've loaded all available crates
    if (newCrates.length < ITEMS_PER_PAGE) {
      allLoaded.value = true
    }
  }
}

// Handle scroll event for infinite scrolling
function handleScroll(event: Event) {
  const target = event.target as HTMLElement

  // If we're in search mode, don't use infinite scroll
  if (searchText.value !== "") return

  // Calculate if we're near the bottom (within 200px)
  const scrollTop = target.scrollTop
  const scrollHeight = target.scrollHeight
  const clientHeight = target.clientHeight

  const scrollBottom = scrollHeight - scrollTop - clientHeight
  const isNearBottom = scrollBottom < 200

  if (isNearBottom && !isLoading.value && !allLoaded.value) {
    loadMoreCrates()
  }
}

// Refresh crates (used when changing filters)
function refreshCrates() {
  crates.value = []
  currentPage.value = 0
  allLoaded.value = false

  // Reset scroll position
  if (scrollContainer.value) {
    scrollContainer.value.scrollTop = 0
  }

  loadMoreCrates()
}

// Search crates by name
async function handleSearch(query: string) {
  const searchQuery = query.trim()

  if (!searchQuery) {
    refreshCrates()
    return
  }

  isLoading.value = true
  allLoaded.value = false

  // Reset scroll position
  if (scrollContainer.value) {
    scrollContainer.value.scrollTop = 0
  }

  const result = await crateService.searchCrates(searchQuery, store.searchCache)

  isLoading.value = false

  if (isSuccess(result)) {
    crates.value = result.data.crates
    allLoaded.value = true // Search results are all loaded at once
  } else {
    crates.value = []
    allLoaded.value = true
  }
}
</script>

<style scoped>
.main-container {
  display: flex;
  flex-direction: column;
  height: calc(100vh - 64px);
  /* Adjust for app bar height */
  overflow: hidden;
  position: relative;
}

/* Search Card */
.search-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
}

/* Search Field */
.search-field :deep(.v-field) {
  background: rgb(var(--v-theme-surface));
  border-radius: 8px;
}

.search-field :deep(.v-field__input) {
  padding-top: 10px;
  padding-bottom: 10px;
  min-height: 44px;
}

.search-field :deep(.v-field__prepend-inner .v-icon) {
  color: rgb(var(--v-theme-primary));
  opacity: 0.8;
}

/* Switch Label */
.switch-label {
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface));
}

.info-icon {
  color: rgb(var(--v-theme-primary));
  opacity: 0.7;
}

.content-container {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  position: relative;
  padding: 0;
  margin: 0 12px 12px 12px;
}

/* Improve scrollbar appearance */
.content-container::-webkit-scrollbar {
  width: 6px;
}

.content-container::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.2);
  border-radius: 3px;
}

.content-container::-webkit-scrollbar-track {
  background: transparent;
}

/* Dark mode adjustments */
:deep(.v-theme--dark) .content-container::-webkit-scrollbar-thumb {
  background-color: rgba(255, 255, 255, 0.2);
}

/* Mobile adjustments */
@media (max-width: 600px) {
  .main-container {
    height: calc(100vh - 56px);
    /* Adjust for smaller mobile app bar */
  }

  .content-container {
    margin: 0 8px 8px 8px;
  }
}
</style>
