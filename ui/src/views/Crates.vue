<template>
  <v-container fluid class="pa-0 main-container">
    <!-- Search Header -->
    <v-card class="search-card pa-4 ma-3" elevation="0" rounded="lg">
      <v-row no-gutters align="center">
        <v-col cols="12" md="8" lg="6" class="pr-md-4">
          <v-text-field v-model="searchText" placeholder="Search for crates" variant="outlined" density="comfortable"
            hide-details prepend-inner-icon="mdi-magnify" color="primary" data-testid="crates-search"
            @keyup.enter="handleSearch(searchText)" class="search-field" rounded="lg"></v-text-field>
        </v-col>

        <v-col cols="12" md="4" lg="6" class="mt-3 mt-md-0 d-flex align-center">
          <v-switch v-model="store.searchCache" color="primary" hide-details data-testid="crates-proxy-toggle"
            @update:model-value="refreshCrates()">
            <template v-slot:label>
              <div class="d-flex align-center" data-testid="crates-proxy-label">
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

    <!-- Result count and sort order -->
    <div v-if="total !== null" class="list-toolbar mx-3 mb-3">
      <span class="text-body-medium result-count" data-testid="crates-result-count">{{ resultCountText }}</span>
      <v-select v-if="!activeQuery" v-model="sort" :items="SORT_OPTIONS" density="compact" variant="outlined"
        hide-details class="sort-select" aria-label="Sort crates" data-testid="crates-sort"
        @update:model-value="handleSortChange"></v-select>
      <span v-else class="text-body-small result-count">Best matches first</span>
    </div>

    <div class="content-container" data-testid="crates-list">
      <!-- Empty State -->
      <v-card v-if="crates.length === 0 && !isLoading" class="pa-6 text-center mx-auto my-8" max-width="500"
        variant="outlined" data-testid="crates-empty-state">
        <v-card-title class="text-title-large font-weight-medium">No crates found</v-card-title>
        <v-card-text>
          <p>To learn how to publish crates to <strong>Kellnr</strong>, read the
            <a href="https://kellnr.io/documentation" target="_blank" rel="noopener noreferrer" class="text-decoration-none font-weight-medium text-primary">
              documentation
            </a>
          </p>
          <v-icon icon="mdi-package-variant" size="x-large" color="grey-lighten-1" class="my-4"></v-icon>
        </v-card-text>
      </v-card>

      <crate-card v-for="crate in crates" :key="`${crate.name}-${crate.version}`" :crate="crate.name"
        :version="crate.version" :updated="crate.date" :downloads="crate.total_downloads" :desc="crate.description"
        :doc-link="crate.documentation" :is-cache="crate.is_cache"></crate-card>

      <!-- Loads the next page when it scrolls into view -->
      <div ref="loadMoreSentinel" class="load-more-sentinel" aria-hidden="true"></div>

      <!-- Loading Indicator -->
      <div v-if="isLoading" class="text-center my-4 pb-4" data-testid="crates-loading">
        <v-progress-circular indeterminate color="primary" :size="40"></v-progress-circular>
        <div class="text-body-medium mt-2">Loading crates...</div>
      </div>

      <!-- End of Results -->
      <div v-if="allLoaded && crates.length > 0" class="text-center my-6 text-body-medium text-grey"
        data-testid="crates-end-of-results">
        End of crates
      </div>
    </div>
  </v-container>
</template>

<script setup lang="ts">
import { computed, onBeforeMount, onBeforeUnmount, onMounted, ref } from "vue"
import CrateCard from "../components/CrateCard.vue"
import type { CrateOverview } from "../types/crate_overview"
import type { CrateSort } from "../types/api"
import { crateService } from "../services"
import { isSuccess } from "../services/api"
import { useRouter } from "vue-router"
import { useStore } from "../store/store"
import { formatNumber } from "../common/format"

// Constants
const ITEMS_PER_PAGE = 20
const SORT_OPTIONS: { title: string, value: CrateSort }[] = [
  { title: "Name", value: "name" },
  { title: "Recently updated", value: "updated" },
  { title: "Most downloaded", value: "downloads" },
]

// State
const crates = ref<CrateOverview[]>([])
const currentPage = ref(0)
const isLoading = ref(false)
const allLoaded = ref(false)
const total = ref<number | null>(null)
const searchText = ref("")
// The query the currently displayed list belongs to. Empty means "browse all".
// Kept apart from `searchText` so typing does not change what the next page is.
const activeQuery = ref("")
const sort = ref<CrateSort>("name")
const loadMoreSentinel = ref<HTMLElement | null>(null)
const router = useRouter()
const store = useStore()
let observer: IntersectionObserver | undefined

const resultCountText = computed(() => {
  const count = total.value ?? 0
  const noun = count === 1 ? "crate" : "crates"
  return activeQuery.value
    ? `${formatNumber(count)} ${noun} found for "${activeQuery.value}"`
    : `${formatNumber(count)} ${noun}`
})

// Initial setup
onBeforeMount(() => {
  const query = router.currentRoute.value.query
  if (query.search) {
    searchText.value = query.search as string
    activeQuery.value = searchText.value.trim()
  }
  const requestedSort = SORT_OPTIONS.find(option => option.value === query.sort)
  if (requestedSort) {
    sort.value = requestedSort.value
  }
})

onMounted(() => {
  loadMoreCrates()

  // Load the next page shortly before the end of the list becomes visible
  observer = new IntersectionObserver(entries => {
    if (entries.some(entry => entry.isIntersecting)) {
      loadMoreCrates()
    }
  }, { rootMargin: "200px" })
  if (loadMoreSentinel.value) {
    observer.observe(loadMoreSentinel.value)
  }
})

onBeforeUnmount(() => {
  observer?.disconnect()
})

// Load more crates for infinite scrolling. Search and the full crate list are
// paginated the same way, so both are loaded through here.
async function loadMoreCrates() {
  if (isLoading.value || allLoaded.value) return

  isLoading.value = true

  const result = activeQuery.value
    ? await crateService.searchCrates(
      activeQuery.value,
      currentPage.value,
      ITEMS_PER_PAGE,
      store.searchCache
    )
    : await crateService.getCrates(
      currentPage.value,
      ITEMS_PER_PAGE,
      store.searchCache,
      sort.value
    )

  isLoading.value = false

  if (isSuccess(result)) {
    const newCrates = result.data.crates

    // Add crates to the list
    crates.value = [...crates.value, ...newCrates]
    total.value = result.data.total

    // Increment page for next load
    currentPage.value = result.data.page + 1

    // Check if we've loaded all available crates
    if (newCrates.length < ITEMS_PER_PAGE) {
      allLoaded.value = true
    } else {
      // The sentinel may still be in view after a short page, which does not
      // fire the observer again, so check it directly.
      loadMoreIfSentinelVisible()
    }
  } else {
    // Stop here, otherwise scrolling retries the failed request indefinitely.
    allLoaded.value = true
  }
}

function loadMoreIfSentinelVisible() {
  requestAnimationFrame(() => {
    const rect = loadMoreSentinel.value?.getBoundingClientRect()
    if (rect && rect.top < window.innerHeight + 200) {
      loadMoreCrates()
    }
  })
}

// Refresh crates (used when changing filters)
function refreshCrates() {
  crates.value = []
  currentPage.value = 0
  allLoaded.value = false
  total.value = null
  window.scrollTo({ top: 0 })

  loadMoreCrates()
}

// Keep search and sort order in the URL, so the list can be linked and
// survives a reload.
function updateUrl() {
  router.replace({
    query: {
      ...(activeQuery.value ? { search: activeQuery.value } : {}),
      ...(sort.value !== "name" ? { sort: sort.value } : {}),
    },
  })
}

// Search crates by name. An empty query goes back to the full crate list.
function handleSearch(query: string) {
  activeQuery.value = query.trim()
  updateUrl()
  refreshCrates()
}

function handleSortChange() {
  updateUrl()
  refreshCrates()
}
</script>

<style scoped>
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

/* Result count and sort order */
.list-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 8px;
}

.result-count {
  color: rgb(var(--v-theme-on-surface-variant));
}

.sort-select {
  flex: 0 0 200px;
}

.sort-select :deep(.v-field) {
  background: rgb(var(--v-theme-surface));
}

/* Same side margins as the search card, so both line up */
.content-container {
  margin: 0 12px 12px 12px;
}

.load-more-sentinel {
  height: 1px;
}
</style>
