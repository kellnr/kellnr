<template>
  <v-container fluid class="pa-0">
    <PageHeader title="Search" icon="mdi-magnify" class="mb-4" />

    <!-- Search, sort order and the proxy switch in one row -->
    <div class="crates-toolbar mb-2">
      <v-text-field v-model="searchText" placeholder="Search for crates" variant="outlined" density="comfortable"
        hide-details prepend-inner-icon="mdi-magnify" color="primary" data-testid="crates-search"
        :loading="isLoading" aria-label="Search for crates"
        data-search-shortcut @keyup.enter="searchNow" class="search-field" bg-color="surface">
        <template v-slot:append-inner>
          <kbd class="shortcut-hint" title="Press / or S to search">/</kbd>
        </template>
      </v-text-field>

      <!-- Stays in place while searching, so the row does not shift -->
      <v-tooltip location="top" text="Search results are sorted by relevance" :disabled="!activeQuery">
        <template v-slot:activator="{ props: tooltipProps }">
          <div v-bind="tooltipProps" class="sort-select" data-testid="crates-sort-wrapper">
            <v-select :model-value="activeQuery ? RELEVANCE : sort" :items="sortItems" :disabled="!!activeQuery"
              density="comfortable" variant="outlined" hide-details bg-color="surface" aria-label="Sort crates"
              data-testid="crates-sort" @update:model-value="handleSortChange"></v-select>
          </div>
        </template>
      </v-tooltip>

      <div v-if="proxyEnabled" class="d-flex align-center flex-grow-0">
        <v-switch :model-value="proxy" color="primary" hide-details density="compact" class="flex-grow-0 proxy-switch"
          data-testid="crates-proxy-toggle" @update:model-value="handleProxyChange">
          <template v-slot:label>
            <span class="switch-label" data-testid="crates-proxy-label">Crates proxy</span>
          </template>
        </v-switch>
        <v-tooltip location="top" text="Display crates from the crates.io proxy">
          <template v-slot:activator="{ props: tooltipProps }">
            <v-btn v-bind="tooltipProps" icon="mdi-information-outline" variant="text" size="small"
              class="info-icon ms-1" aria-label="About the crates proxy" data-testid="crates-proxy-info"></v-btn>
          </template>
        </v-tooltip>
      </div>
    </div>

    <!-- Result count and the active keyword or category filter -->
    <div class="result-line mb-3">
      <template v-if="total !== null">
        <span class="text-body-medium result-count" data-testid="crates-result-count">{{ resultCountText }}</span>
        <span v-if="activeQuery" class="text-body-medium result-count">, best matches first</span>
      </template>
      <v-chip v-if="tags.category" size="small" closable prepend-icon="mdi-folder-outline" class="ms-2"
        close-label="Remove category filter" data-testid="crates-category-filter" @click:close="clearTags">
        {{ tags.category }}
      </v-chip>
      <v-chip v-if="tags.keyword" size="small" closable class="ms-2" close-label="Remove keyword filter"
        data-testid="crates-keyword-filter" @click:close="clearTags">
        #{{ tags.keyword }}
      </v-chip>
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
        :doc-link="crate.documentation" :repository="crate.repository" :is-cache="crate.is_cache"></crate-card>

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
import { computed, onBeforeMount, onBeforeUnmount, onMounted, ref, watch } from "vue"
import CrateCard from "../components/CrateCard.vue"
import { PageHeader } from "../components/common"
import type { CrateOverview } from "../types/crate_overview"
import type { CrateSort, CrateTagFilter } from "../types/api"
import { crateService, settingsService } from "../services"
import { isSuccess } from "../services/api"
import { useRoute, useRouter } from "vue-router"
import { formatNumber } from "../common/format"
import { debounce } from "../common/debounce"
import { buildCrateListQuery, DEFAULT_CRATE_SORT, parseCrateProxy, parseCrateSort } from "../common/crateListQuery"

// Constants
const ITEMS_PER_PAGE = 20
const SEARCH_DELAY_MS = 300
const SORT_OPTIONS: { title: string, value: CrateSort }[] = [
  { title: "Name", value: "name" },
  { title: "Recently updated", value: "updated" },
  { title: "Most downloaded", value: "downloads" },
]
// Shown in the disabled sort field while a search is active
const RELEVANCE = "relevance"

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
const sort = ref<CrateSort>(DEFAULT_CRATE_SORT)
// Keyword and category filter, set by the links on a crate page
const tags = ref<CrateTagFilter>({})
// Lists the cached crates.io crates instead of the registry crates
const proxy = ref(false)
// Hides the proxy switch until the server confirms that the proxy is enabled
const proxyEnabled = ref(false)
const loadMoreSentinel = ref<HTMLElement | null>(null)
const router = useRouter()
const route = useRoute()
let observer: IntersectionObserver | undefined
// Increased on every refresh, so responses for an older list are dropped
let listGeneration = 0

const sortItems = computed(() =>
  activeQuery.value ? [{ title: "Relevance", value: RELEVANCE }] : SORT_OPTIONS
)

const resultCountText = computed(() => {
  const count = total.value ?? 0
  const noun = count === 1 ? "crate" : "crates"
  if (activeQuery.value) {
    return `${formatNumber(count)} ${noun} found for "${activeQuery.value}"`
  }
  if (tags.value.keyword || tags.value.category) {
    return `${formatNumber(count)} ${noun} with`
  }
  return `${formatNumber(count)} ${noun}`
})

function tagsFromRoute(): CrateTagFilter {
  const keyword = route.query.keyword?.toString()
  const category = route.query.category?.toString()
  return {
    ...(keyword ? { keyword } : {}),
    ...(category ? { category } : {}),
  }
}

// Initial setup
onBeforeMount(() => {
  const query = router.currentRoute.value.query
  if (query.search) {
    searchText.value = query.search as string
    activeQuery.value = searchText.value.trim()
  }
  sort.value = parseCrateSort(query.sort) ?? DEFAULT_CRATE_SORT
  proxy.value = parseCrateProxy(query.proxy)
  // A search and a tag filter are not combined, the search wins
  if (!activeQuery.value) {
    tags.value = tagsFromRoute()
  }
})

// A link to another keyword or category on this page, or going back to the
// previous filter, reuses this view
watch(() => [route.query.keyword, route.query.category], () => {
  const next = tagsFromRoute()
  if (next.keyword === tags.value.keyword && next.category === tags.value.category) return
  tags.value = next
  if (next.keyword || next.category) {
    searchText.value = ""
    activeQuery.value = ""
  }
  refreshCrates()
})

// A link to the registry or the cached crates while this view is open
watch(() => route.query.proxy, value => {
  const next = parseCrateProxy(value)
  if (next === proxy.value) return
  proxy.value = next
  refreshCrates()
})

const searchWhileTyping = debounce((text: string) => handleSearch(text), SEARCH_DELAY_MS)

watch(searchText, text => {
  // Also covers text set by this view itself, which already matches the list
  if (text.trim() === activeQuery.value) {
    searchWhileTyping.cancel()
    return
  }
  searchWhileTyping(text)
})

onMounted(() => {
  loadMoreCrates()
  loadProxyEnabled()

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
  searchWhileTyping.cancel()
})

async function loadProxyEnabled() {
  const result = await settingsService.getProxyEnabled()
  if (!isSuccess(result)) return
  proxyEnabled.value = result.data.enabled
  // A link to the cached crates would otherwise keep the hidden switch on
  if (!proxyEnabled.value && proxy.value) {
    proxy.value = false
    updateUrl()
    refreshCrates()
  }
}

// Load more crates for infinite scrolling. Search and the full crate list are
// paginated the same way, so both are loaded through here.
async function loadMoreCrates() {
  if (isLoading.value || allLoaded.value) return

  isLoading.value = true
  const generation = listGeneration

  const result = activeQuery.value
    ? await crateService.searchCrates(
      activeQuery.value,
      currentPage.value,
      ITEMS_PER_PAGE,
      proxy.value
    )
    : await crateService.getCrates(
      currentPage.value,
      ITEMS_PER_PAGE,
      proxy.value,
      sort.value,
      tags.value
    )

  // The list was refreshed meanwhile, the refresh loads its own first page
  if (generation !== listGeneration) return

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
  listGeneration++
  crates.value = []
  currentPage.value = 0
  allLoaded.value = false
  isLoading.value = false
  total.value = null
  window.scrollTo({ top: 0 })

  loadMoreCrates()
}

function updateUrl() {
  router.replace({
    query: buildCrateListQuery({
      search: activeQuery.value,
      sort: sort.value,
      tags: tags.value,
      proxy: proxy.value,
    }),
  })
}

// Search crates by name. An empty query goes back to the full crate list.
function handleSearch(query: string) {
  const trimmed = query.trim()
  const hasTags = Boolean(tags.value.keyword || tags.value.category)
  if (trimmed === activeQuery.value && !hasTags) return
  activeQuery.value = trimmed
  tags.value = {}
  updateUrl()
  refreshCrates()
}

// Enter searches at once instead of waiting for the typing delay
function searchNow() {
  searchWhileTyping.cancel()
  handleSearch(searchText.value)
}

function clearTags() {
  tags.value = {}
  updateUrl()
  refreshCrates()
}

function handleProxyChange(value: boolean | null) {
  const next = value === true
  if (next === proxy.value) return
  proxy.value = next
  updateUrl()
  refreshCrates()
}

function handleSortChange(value: unknown) {
  const next = parseCrateSort(value)
  if (!next || next === sort.value) return
  sort.value = next
  updateUrl()
  refreshCrates()
}
</script>

<style scoped>
.crates-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px 16px;
}

.search-field {
  flex: 1 1 320px;
}

.search-field :deep(.v-field__prepend-inner .v-icon) {
  color: rgb(var(--v-theme-on-surface-variant));
}

.sort-select {
  flex: 0 0 200px;
}

/* The track sticks out of the switch, this keeps it inside the page margin */
.proxy-switch {
  padding-inline-start: 4px;
}

/* Sort order and proxy switch share a row on a phone */
@media (max-width: 600px) {
  .sort-select {
    flex-basis: 160px;
  }
}

/* Switch Label */
.switch-label {
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface));
}

.info-icon {
  color: rgb(var(--v-theme-on-surface-variant));
}

/* Keeps its height while the count loads, so the list does not jump */
.result-line {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  min-height: 24px;
}

.result-count {
  color: rgb(var(--v-theme-on-surface-variant));
}

.load-more-sentinel {
  height: 1px;
}
</style>
