<template>
  <v-container fluid class="pa-0">
    <!-- Hero Search Section -->
    <v-row class="mb-2">
      <v-col cols="12">
        <div class="text-center mb-4">
          <h1 class="text-headline-large text-md-display-small font-weight-bold welcome-title mb-2">
            Welcome to Kellnr
          </h1>
          <p class="text-body-large welcome-subtitle">
            Your private Rust crate registry
          </p>
        </div>
        <v-row>
          <v-col cols="12" sm="10" md="8" lg="6" class="mx-auto">
            <div class="search-wrapper">
              <v-text-field v-model="searchText" placeholder="Search for crates" variant="outlined"
                density="comfortable" prepend-inner-icon="mdi-magnify" hide-details data-testid="landing-search"
                @keyup.enter="searchCrates()" class="search-field" bg-color="surface"></v-text-field>
            </div>
          </v-col>
        </v-row>
      </v-col>
    </v-row>

    <!-- Loading State -->
    <v-row v-if="!statistics" class="my-8">
      <v-col cols="12" class="text-center">
        <v-progress-circular indeterminate color="primary" size="60" width="6"
          data-testid="landing-loading"></v-progress-circular>
        <div class="mt-4 text-body-large loading-text">Loading registry statistics...</div>
      </v-col>
    </v-row>

    <!-- Dashboard Content -->
    <template v-else>
      <!-- Registry statistics -->
      <v-row class="mb-2">
        <v-col cols="12" md="4">
          <hero-stat-card
            :num="statistics.num_crates"
            icon="mdi-package-variant-closed"
            text="Total crates"
            subtitle="Published to your registry"
            :onClick="navigateToCrates">
          </hero-stat-card>
        </v-col>
        <v-col cols="12" md="4">
          <hero-stat-card
            :num="statistics.num_crate_versions"
            icon="mdi-tag-multiple"
            text="Total versions"
            subtitle="Across all crates">
          </hero-stat-card>
        </v-col>
        <v-col cols="12" md="4">
          <hero-stat-card
            :num="statistics.num_crate_downloads"
            icon="mdi-download"
            text="Total downloads"
            subtitle="From your registry">
          </hero-stat-card>
        </v-col>
      </v-row>

      <!-- An empty registry points to the setup guide -->
      <v-row v-if="statistics.num_crates === 0" class="mb-6">
        <v-col cols="12">
          <v-card class="empty-registry-card pa-6 text-center" elevation="0" data-testid="landing-empty">
            <v-icon icon="mdi-package-variant" size="40" color="primary" class="mb-2"></v-icon>
            <div class="text-title-large font-weight-medium mb-1">No crates yet</div>
            <p class="text-body-medium mb-0">
              To learn how to publish crates to Kellnr, read the
              <a href="https://kellnr.io/documentation" target="_blank" rel="noopener noreferrer"
                class="text-primary font-weight-medium text-decoration-none">documentation</a>.
            </p>
          </v-card>
        </v-col>
      </v-row>

      <!-- Recently updated and most downloaded crates -->
      <v-row v-else class="mb-6">
        <v-col cols="12" :md="topCrates.length > 0 ? 6 : 12">
          <crate-list-card title="Recently updated" icon="mdi-update" :crates="recentCrates" sort="updated"
            test-id="landing-recent-crates" row-test-id="recent-crate"></crate-list-card>
        </v-col>
        <v-col v-if="topCrates.length > 0" cols="12" md="6">
          <crate-list-card title="Most downloaded" icon="mdi-trending-up" :crates="topCrates" sort="downloads"
            test-id="landing-top-crates" row-test-id="top-crate"></crate-list-card>
        </v-col>
      </v-row>

      <!-- Cached Crates Section -->
      <v-row v-if="statistics.proxy_enabled" class="mb-6">
        <v-col cols="12">
          <div class="section-divider mb-5">
            <span class="section-divider-text" data-testid="landing-cached-section">Crates.io proxy cache</span>
          </div>
        </v-col>

        <v-col cols="12" sm="6" md="4">
          <hero-stat-card :num="statistics.num_proxy_crates" icon="mdi-package-variant" text="Cached crates"
            compact test-id-prefix="stat-card" :onClick="navigateToCachedCrates"></hero-stat-card>
        </v-col>
        <v-col cols="12" sm="6" md="4">
          <hero-stat-card :num="statistics.num_proxy_crate_versions" icon="mdi-tag-outline" text="Cached versions"
            compact test-id-prefix="stat-card"></hero-stat-card>
        </v-col>
        <v-col cols="12" sm="6" md="4">
          <hero-stat-card :num="statistics.num_proxy_crate_downloads" icon="mdi-cloud-download"
            text="Cached downloads" compact test-id-prefix="stat-card"></hero-stat-card>
        </v-col>
      </v-row>
    </template>
  </v-container>
</template>

<script setup lang="ts">
import { onBeforeMount, ref } from "vue"
import HeroStatCard from '../components/HeroStatCard.vue'
import CrateListCard from '../components/CrateListCard.vue'
import type { Statistics } from '../types/statistics'
import type { CrateOverview } from '../types/crate_overview'
import { crateService } from '../services'
import { isSuccess } from '../services/api'
import router from '../router'
import { useStore } from '../store/store'

// Crates shown in the "Recently updated" and "Most downloaded" lists
const LIST_SIZE = 5
// A ranking of fewer crates is mostly empty space
const MIN_RANKED_CRATES = 3

const statistics = ref<Statistics>()
const recentCrates = ref<CrateOverview[]>([])
const topCrates = ref<CrateOverview[]>([])
const searchText = ref("")
const store = useStore()

onBeforeMount(async () => {
  const [stats, recent, top] = await Promise.all([
    crateService.getStatistics(),
    crateService.getCrates(0, LIST_SIZE, false, 'updated'),
    crateService.getCrates(0, LIST_SIZE, false, 'downloads'),
  ])
  if (isSuccess(recent)) {
    recentCrates.value = recent.data.crates
  }
  if (isSuccess(top)) {
    // A ranking of crates nobody downloaded yet says nothing
    const downloaded = top.data.crates.filter(crate => crate.total_downloads > 0)
    topCrates.value = downloaded.length >= MIN_RANKED_CRATES ? downloaded : []
  }
  if (isSuccess(stats)) {
    statistics.value = stats.data
  }
})

function searchCrates() {
  if (searchText.value.length > 0) {
    router.push({ path: '/crates', query: { search: searchText.value } })
  }
}

function navigateToCrates() {
  store.searchCache = false
  router.push({ path: '/crates' })
}

function navigateToCachedCrates() {
  store.searchCache = true
  router.push({ path: '/crates' })
}

</script>

<style scoped>
.welcome-title {
  color: rgb(var(--v-theme-on-background));
  letter-spacing: -0.02em;
}

.welcome-subtitle {
  color: rgb(var(--v-theme-on-surface-variant));
}

/* Mobile-specific styles */
@media (max-width: 600px) {
  .welcome-title {
    font-size: 1.5rem !important;
  }

  .welcome-subtitle {
    font-size: 0.9rem !important;
  }
}

.loading-text {
  color: rgba(var(--v-theme-on-background), 0.7);
}

.search-wrapper {
  max-width: 800px;
  margin: 0 auto;
}

.search-field {
  font-size: 1.1rem;
}

.section-divider {
  display: flex;
  align-items: center;
  text-align: center;
}

.section-divider::before,
.section-divider::after {
  content: '';
  flex: 1;
  border-bottom: 1px solid rgba(var(--v-theme-on-background), 0.12);
}

.section-divider-text {
  padding: 0 16px;
  font-size: 0.875rem;
  font-weight: 500;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  color: rgba(var(--v-theme-on-background), 0.5);
}

.empty-registry-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
}
</style>
