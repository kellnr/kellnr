<template>
  <v-container fluid class="landing-container pa-4">
    <!-- Hero Section -->
    <v-row>
      <v-col cols="12">
        <v-card class="hero-card text-center py-6 px-4 mb-2" flat>
          <!-- Search Box -->
          <v-row>
            <v-col cols="12" sm="10" md="8" lg="6" class="mx-auto">
              <!-- Updated search field that works in both light and dark mode -->
              <div class="search-wrapper">
                <v-text-field v-model="searchText" placeholder="Search for crates" variant="outlined"
                  density="comfortable" prepend-inner-icon="mdi-magnify" hide-details @keyup.enter="searchCrates()"
                  class="search-field" bg-color="surface" rounded="pill"></v-text-field>
              </div>
            </v-col>
          </v-row>
        </v-card>
      </v-col>
    </v-row>

    <!-- Overview Section -->
    <v-row class="mb-3">
      <v-col cols="12">
        <div class="d-flex align-center mb-4">
          <div class="section-line mr-4"></div>
          <h2 class="text-h4 font-weight-bold mb-0">Registry Overview</h2>
          <div class="section-line ml-4"></div>
        </div>
      </v-col>
    </v-row>

    <!-- Loading State -->
    <v-row v-if="!statistics" class="my-8">
      <v-col cols="12" class="text-center">
        <v-progress-circular indeterminate color="primary" size="60" width="6"></v-progress-circular>
        <div class="mt-4 text-body-1">Loading registry statistics...</div>
      </v-col>
    </v-row>

    <!-- Statistics Cards Section -->
    <template v-else>
      <!-- Primary Stats -->
      <v-row class="justify-center mb-8">
        <v-col cols="12" sm="6" md="4" xl="3">
          <statistics-card :num="statistics.num_crates" icon="mdi-package-variant-closed" :text="'Total Crates'"
            category="primary"></statistics-card>
        </v-col>
        <v-col cols="12" sm="6" md="4" xl="3">
          <statistics-card :num="statistics.num_crate_versions" icon="mdi-source-branch" :text="'Total Versions'"
            category="primary"></statistics-card>
        </v-col>
        <v-col cols="12" sm="6" md="4" xl="3">
          <statistics-card :num="statistics.num_crate_downloads" icon="mdi-cloud-download" :text="'Total Downloads'"
            category="primary"></statistics-card>
        </v-col>
        <v-col v-if="statistics.last_updated_crate" cols="12" sm="6" md="4" xl="3">
          <statistics-card :num="statistics.last_updated_crate[0]" icon="mdi-calendar-clock"
            :text="'Updated ' + statistics.last_updated_crate[1]" category="secondary"></statistics-card>
        </v-col>
      </v-row>

      <!-- Top Crates Section -->
      <v-row v-if="statistics.top_crates.first[1] > 0" class="mb-6">
        <v-col cols="12">
          <div class="d-flex align-center mb-4">
            <div class="section-line mr-4"></div>
            <h2 class="text-h5 font-weight-bold mb-0">
              <v-icon icon="mdi-star" color="amber-darken-1" class="mr-2"></v-icon>
              Top Downloaded Crates
            </h2>
            <div class="section-line ml-4"></div>
          </div>
        </v-col>

        <v-col v-if="statistics.top_crates.first[1] > 0" cols="12" sm="6" md="4">
          <statistics-card :num="statistics.top_crates.first[1]" icon="mdi-medal" :text="statistics.top_crates.first[0]"
            category="gold" iconColor="#FFD700"></statistics-card>
        </v-col>
        <v-col v-if="statistics.top_crates.second[1] > 0" cols="12" sm="6" md="4">
          <statistics-card :num="statistics.top_crates.second[1]" icon="mdi-medal"
            :text="statistics.top_crates.second[0]" category="silver" iconColor="#C0C0C0"></statistics-card>
        </v-col>
        <v-col v-if="statistics.top_crates.third[1] > 0" cols="12" sm="6" md="4">
          <statistics-card :num="statistics.top_crates.third[1]" icon="mdi-medal" :text="statistics.top_crates.third[0]"
            category="bronze" iconColor="#CD7F32"></statistics-card>
        </v-col>
      </v-row>

      <!-- Proxy Stats Section -->
      <v-row v-if="statistics.proxy_enabled" class="mb-6">
        <v-col cols="12">
          <div class="d-flex align-center mb-4">
            <div class="section-line mr-4"></div>
            <h2 class="text-h5 font-weight-bold mb-0">
              <v-icon icon="mdi-cloud-sync" color="indigo" class="mr-2"></v-icon>
              Cached Crates
            </h2>
            <div class="section-line ml-4"></div>
          </div>
        </v-col>

        <v-col cols="12" sm="6" md="4">
          <statistics-card :num="statistics.num_proxy_crates" icon="mdi-cube-outline" :text="'Cached Crates'"
            category="cached"></statistics-card>
        </v-col>
        <v-col cols="12" sm="6" md="4">
          <statistics-card :num="statistics.num_proxy_crate_versions" icon="mdi-source-branch" :text="'Cached Versions'"
            category="cached"></statistics-card>
        </v-col>
        <v-col cols="12" sm="6" md="4">
          <statistics-card :num="statistics.num_proxy_crate_downloads" icon="mdi-cloud-download"
            :text="'Cached Downloads'" category="cached"></statistics-card>
        </v-col>
      </v-row>
    </template>
  </v-container>
</template>

<script setup lang="ts">
import axios from 'axios';
import { onBeforeMount, ref } from "vue";
import { STATISTICS } from '../remote-routes';
import StatisticsCard from '../components/StatisticsCard.vue';
import type { Statistics } from '../types/statistics';
import router from '../router';
import { useTheme } from 'vuetify';

const statistics = ref<Statistics>();
const searchText = ref("");
const theme = useTheme();

onBeforeMount(() => {
  axios.get(STATISTICS).then((response) => {
    statistics.value = response.data;
  });
});

function searchCrates() {
  if (searchText.value.length > 0) {
    router.push({ path: '/crates', query: { search: searchText.value } });
  }
}
</script>

<style scoped>
.landing-container {
  max-width: 1400px;
  margin: 0 auto;
}

.hero-card {
  background: var(--v-theme-surface);
  border-radius: 16px;
  margin-bottom: 2rem;
  border: 1px solid var(--v-theme-outline-variant);
  position: relative;
  overflow: hidden;
}

/* Hero card overlay for light theme */
.hero-card::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(135deg, var(--v-theme-primary-lighten-5, rgba(240, 240, 255, 0.2)) 0%,
      var(--v-theme-primary-lighten-4, rgba(225, 235, 255, 0.2)) 100%);
  opacity: 0.7;
  z-index: 0;
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

/* Customize text field for better appearance */
.search-field :deep(.v-field__input) {
  padding-top: 12px;
  padding-bottom: 12px;
  font-weight: 400;
}

.search-field :deep(.v-field__outline) {
  opacity: 0.8;
}

.section-line {
  height: 1px;
  background: linear-gradient(90deg, transparent, var(--v-theme-outline-variant), transparent);
  flex-grow: 1;
}

/* Dark mode adjustments */
:deep(.v-theme--dark) .hero-card::before {
  background: linear-gradient(135deg,
      rgba(66, 66, 120, 0.3) 0%,
      rgba(50, 60, 100, 0.3) 100%);
  opacity: 1;
}

:deep(.v-theme--dark) .search-wrapper {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
}

:deep(.v-theme--dark) .search-wrapper:hover {
  box-shadow: 0 8px 16px rgba(0, 0, 0, 0.5);
}

:deep(.v-theme--dark) .search-field :deep(.v-field__outline) {
  opacity: 0.6;
}

:deep(.v-theme--dark) .section-line {
  background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.1), transparent);
}
</style>
