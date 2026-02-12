<template>
  <v-container fluid class="landing-container pa-4">
    <!-- Hero Search Section -->
    <v-row class="mb-6">
      <v-col cols="12">
        <div class="text-center mb-4">
          <h1 class="text-h4 text-md-h3 font-weight-bold welcome-title mb-2" :class="{ 'dark-mode': isDark }">
            Welcome to Kellnr
          </h1>
          <p class="text-body-1 welcome-subtitle" :class="{ 'dark-mode': isDark }">
            Your private Rust crate registry
          </p>
        </div>
        <v-row>
          <v-col cols="12" sm="10" md="8" lg="6" class="mx-auto">
            <div class="search-wrapper">
              <v-text-field v-model="searchText" placeholder="Search for crates" variant="outlined"
                density="comfortable" prepend-inner-icon="mdi-magnify" hide-details @keyup.enter="searchCrates()"
                class="search-field" bg-color="surface" rounded="pill"></v-text-field>
            </div>
          </v-col>
        </v-row>
      </v-col>
    </v-row>

    <!-- Loading State -->
    <v-row v-if="!statistics" class="my-8">
      <v-col cols="12" class="text-center">
        <v-progress-circular indeterminate color="primary" size="60" width="6"></v-progress-circular>
        <div class="mt-4 text-body-1 loading-text">Loading registry statistics...</div>
      </v-col>
    </v-row>

    <!-- Dashboard Content -->
    <template v-else>
      <!-- Hero Stats Row - Large Cards -->
      <v-row class="mb-6">
        <v-col cols="12" md="4">
          <hero-stat-card
            :num="statistics.num_crates"
            icon="mdi-package-variant-closed"
            text="Total Crates"
            subtitle="Published to your registry"
            category="primary"
            :onClick="navigateToCrates">
          </hero-stat-card>
        </v-col>
        <v-col cols="12" md="4">
          <hero-stat-card
            :num="statistics.num_crate_versions"
            icon="mdi-tag-multiple"
            text="Total Versions"
            subtitle="Across all crates"
            category="secondary">
          </hero-stat-card>
        </v-col>
        <v-col cols="12" md="4">
          <hero-stat-card
            :num="statistics.num_crate_downloads"
            icon="mdi-download"
            text="Total Downloads"
            subtitle="From your registry"
            category="accent">
          </hero-stat-card>
        </v-col>
      </v-row>

      <!-- Secondary Info Row -->
      <v-row class="mb-8">
        <!-- Last Updated Crate -->
        <v-col v-if="statistics.last_updated_crate" cols="12" md="6" lg="4">
          <recent-crate-card
            :crateName="statistics.last_updated_crate[0]"
            :timeAgo="statistics.last_updated_crate[1]"
            :onClick="() => navigateToCrate(statistics.last_updated_crate![0])">
          </recent-crate-card>
        </v-col>

        <!-- Top Downloaded Crates - Compact -->
        <v-col v-if="statistics.top_crates.first[1] > 0" cols="12" md="6" lg="8">
          <v-card class="top-crates-card h-100" elevation="2" rounded="xl">
            <v-card-text class="pa-4">
              <div class="d-flex align-center mb-3">
                <v-icon icon="mdi-trophy" color="amber-darken-1" size="small" class="mr-2"></v-icon>
                <span class="text-subtitle-2 font-weight-bold section-title">Top Downloaded</span>
              </div>
              <div class="d-flex flex-wrap gap-3">
                <div v-if="statistics.top_crates.first[1] > 0" class="top-crate-item">
                  <v-chip color="amber" variant="flat" size="small" class="mr-2">
                    <v-icon icon="mdi-medal" start size="small"></v-icon>
                    1st
                  </v-chip>
                  <span class="crate-name-text">{{ statistics.top_crates.first[0] }}</span>
                  <span class="download-count">({{ statistics.top_crates.first[1].toLocaleString() }})</span>
                </div>
                <div v-if="statistics.top_crates.second[1] > 0" class="top-crate-item">
                  <v-chip color="grey-lighten-1" variant="flat" size="small" class="mr-2">
                    <v-icon icon="mdi-medal" start size="small"></v-icon>
                    2nd
                  </v-chip>
                  <span class="crate-name-text">{{ statistics.top_crates.second[0] }}</span>
                  <span class="download-count">({{ statistics.top_crates.second[1].toLocaleString() }})</span>
                </div>
                <div v-if="statistics.top_crates.third[1] > 0" class="top-crate-item">
                  <v-chip color="orange-lighten-1" variant="flat" size="small" class="mr-2">
                    <v-icon icon="mdi-medal" start size="small"></v-icon>
                    3rd
                  </v-chip>
                  <span class="crate-name-text">{{ statistics.top_crates.third[0] }}</span>
                  <span class="download-count">({{ statistics.top_crates.third[1].toLocaleString() }})</span>
                </div>
              </div>
            </v-card-text>
          </v-card>
        </v-col>
      </v-row>

      <!-- Cached Crates Section -->
      <v-row v-if="statistics.proxy_enabled" class="mb-6">
        <v-col cols="12">
          <div class="section-divider mb-5">
            <span class="section-divider-text">Crates.io Proxy Cache</span>
          </div>
        </v-col>

        <v-col cols="12" sm="6" md="4">
          <statistics-card :num="statistics.num_proxy_crates" icon="mdi-package-variant" text="Cached Crates"
            category="cached" :onClick="navigateToCachedCrates"></statistics-card>
        </v-col>
        <v-col cols="12" sm="6" md="4">
          <statistics-card :num="statistics.num_proxy_crate_versions" icon="mdi-tag-outline" text="Cached Versions"
            category="cached"></statistics-card>
        </v-col>
        <v-col cols="12" sm="6" md="4">
          <statistics-card :num="statistics.num_proxy_crate_downloads" icon="mdi-cloud-download"
            text="Cached Downloads" category="cached"></statistics-card>
        </v-col>
      </v-row>
    </template>
  </v-container>
</template>

<script setup lang="ts">
import { onBeforeMount, ref, computed } from "vue"
import StatisticsCard from '../components/StatisticsCard.vue'
import HeroStatCard from '../components/HeroStatCard.vue'
import RecentCrateCard from '../components/RecentCrateCard.vue'
import type { Statistics } from '../types/statistics'
import { crateService } from '../services'
import { isSuccess } from '../services/api'
import router from '../router'
import { useStore } from '../store/store'
import { useTheme } from 'vuetify'

const statistics = ref<Statistics>()
const searchText = ref("")
const store = useStore()
const theme = useTheme()

// Computed property to check if dark mode is active
const isDark = computed(() => theme.global.current.value.dark)

onBeforeMount(async () => {
  const result = await crateService.getStatistics()
  if (isSuccess(result)) {
    statistics.value = result.data
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

function navigateToCrate(crateName: string) {
  router.push({ name: 'Crate', query: { name: crateName } })
}
</script>

<style scoped>
.landing-container {
  max-width: 1400px;
  margin: 0 auto;
  padding-bottom: 80px; /* Space for footer */
}

.welcome-title {
  color: #333333;
  position: relative;
  z-index: 1;
  text-shadow: 0 1px 3px rgba(255, 255, 255, 0.5);
}

.welcome-title.dark-mode {
  color: #ffffff;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.7);
}

.welcome-subtitle {
  color: #555555;
  position: relative;
  z-index: 1;
  text-shadow: 0 1px 2px rgba(255, 255, 255, 0.3);
}

.welcome-subtitle.dark-mode {
  color: rgba(255, 255, 255, 0.85);
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.5);
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
  position: relative;
  z-index: 1;
  max-width: 800px;
  margin: 0 auto;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  border-radius: 28px;
  transition: all 0.3s ease;
}

.search-wrapper:hover {
  transform: translateY(-2px);
  box-shadow: 0 8px 16px rgba(0, 0, 0, 0.15);
}

.search-field {
  font-size: 1.1rem;
}

.search-field :deep(.v-field__input) {
  padding-top: 12px;
  padding-bottom: 12px;
  font-weight: 400;
}

.search-field :deep(.v-field__outline) {
  opacity: 0.8;
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

.section-title {
  color: rgba(var(--v-theme-on-background), 0.7);
}

/* Top Crates Card */
.top-crates-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
}

.top-crate-item {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  border-radius: 8px;
  background: rgb(var(--v-theme-surface-variant));
}

.crate-name-text {
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface));
}

.download-count {
  margin-left: 8px;
  font-size: 0.85rem;
  color: rgb(var(--v-theme-on-surface-variant));
}


/* Gap utility for older browsers */
.gap-3 {
  gap: 12px;
}
</style>
