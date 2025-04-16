<template>
  <v-card-text class="pa-0">
    <div class="about-tab-container">
      <!-- Main Metadata Grid -->
      <v-row>
        <!-- Links & Basic Info Section -->
        <v-col cols="12" md="6">
          <v-card class="mb-4 card-surface" elevation="1">
            <v-card-title class="pb-1 card-header">
              <v-icon icon="mdi-information-outline" class="mr-2" />
              Basic Information
            </v-card-title>

            <v-card-text>
              <!-- Links Grid with nicer styling -->
              <div class="info-grid">
                <template v-if="crate.homepage">
                  <div class="info-label">
                    <v-icon icon="mdi-link-variant" size="small" color="primary" class="mr-1" />
                    Homepage
                  </div>
                  <div class="info-content">
                    <a :href="crate.homepage" target="_blank" class="text-decoration-none external-link">
                      {{ crate.homepage }}
                      <v-icon icon="mdi-open-in-new" size="x-small" class="ml-1" />
                    </a>
                  </div>
                </template>

                <template v-if="selectedVersion.license">
                  <div class="info-label">
                    <v-icon icon="mdi-scale-balance" size="small" color="primary" class="mr-1" />
                    License
                  </div>
                  <div class="info-content license-chip">
                    <v-chip size="small" color="primary" variant="flat">
                      {{ selectedVersion.license }}
                    </v-chip>
                  </div>
                </template>

                <template v-if="crate.repository">
                  <div class="info-label">
                    <v-icon icon="mdi-github" size="small" color="primary" class="mr-1" />
                    Repository
                  </div>
                  <div class="info-content">
                    <a :href="crate.repository" target="_blank" class="text-decoration-none external-link">
                      {{ crate.repository }}
                      <v-icon icon="mdi-open-in-new" size="x-small" class="ml-1" />
                    </a>
                  </div>
                </template>

                <template v-if="selectedVersion.yanked === true">
                  <div class="info-label">
                    <v-icon icon="mdi-delete" size="small" color="error" class="mr-1" />
                    Status
                  </div>
                  <div class="info-content">
                    <v-chip size="small" color="error" variant="flat">Yanked</v-chip>
                  </div>
                </template>
              </div>
            </v-card-text>
          </v-card>
        </v-col>

        <!-- Authors Section -->
        <v-col cols="12" md="6">
          <v-card v-if="crate.authors && crate.authors.length > 0" class="mb-4 card-surface" elevation="1">
            <v-card-title class="pb-1 card-header">
              <v-icon icon="mdi-account-multiple" class="mr-2" />
              Authors
            </v-card-title>

            <v-card-text>
              <div class="chip-container">
                <v-chip v-for="(author, i) in crate.authors" :key="`author-${i}`" size="small" variant="flat"
                  class="ma-1" prepend-icon="mdi-account">
                  {{ author }}
                </v-chip>
              </div>
            </v-card-text>
          </v-card>
        </v-col>

        <!-- Owners Section -->
        <v-col cols="12" sm="6" md="4">
          <v-card v-if="sortedOwners.length > 0" class="mb-4 card-surface" elevation="1">
            <v-card-title class="pb-1 card-header">
              <v-icon icon="mdi-shield-account" class="mr-2" />
              Owners
            </v-card-title>

            <v-card-text>
              <div class="chip-container">
                <v-chip v-for="(owner, i) in sortedOwners" :key="`owner-${i}`" size="small" variant="flat" class="ma-1"
                  prepend-icon="mdi-account-key">
                  {{ owner }}
                </v-chip>
              </div>
            </v-card-text>
          </v-card>
        </v-col>

        <!-- Categories Section -->
        <v-col cols="12" sm="6" md="4">
          <v-card v-if="crate.categories && crate.categories.length > 0" class="mb-4 card-surface" elevation="1">
            <v-card-title class="pb-1 card-header">
              <v-icon icon="mdi-folder-multiple" class="mr-2" />
              Categories
            </v-card-title>

            <v-card-text>
              <div class="chip-container">
                <v-chip v-for="(category, i) in crate.categories" :key="`category-${i}`" size="small" color="info"
                  variant="flat" class="ma-1" prepend-icon="mdi-cube">
                  {{ category }}
                </v-chip>
              </div>
            </v-card-text>
          </v-card>
        </v-col>

        <!-- Keywords Section -->
        <v-col cols="12" sm="6" md="4">
          <v-card v-if="crate.keywords && crate.keywords.length > 0" class="mb-4 card-surface" elevation="1">
            <v-card-title class="pb-1 card-header">
              <v-icon icon="mdi-tag-multiple" class="mr-2" />
              Keywords
            </v-card-title>

            <v-card-text>
              <div class="chip-container">
                <v-chip v-for="(keyword, i) in crate.keywords" :key="`keyword-${i}`" size="small" color="success"
                  variant="flat" class="ma-1" prepend-icon="mdi-key">
                  {{ keyword }}
                </v-chip>
              </div>
            </v-card-text>
          </v-card>
        </v-col>
      </v-row>

      <!-- Features Section (Full Width) -->
      <v-card v-if="flattenedFeatures.length > 0" class="mb-4 card-surface" elevation="1">
        <v-card-title class="pb-1 card-header">
          <v-icon icon="mdi-widgets" class="mr-2" />
          Features
        </v-card-title>

        <v-card-text>
          <div class="features-container">
            <v-chip v-for="(feature, i) in flattenedFeatures" :key="`feature-${i}`" size="small" variant="outlined"
              color="primary" class="ma-1">
              <template v-slot:prepend>
                <v-icon icon="mdi-cog" size="small" />
              </template>
              {{ feature }}
            </v-chip>
          </div>
        </v-card-text>
      </v-card>
    </div>
  </v-card-text>
</template>

<script setup lang="ts">
import { defineProps } from 'vue';
import type { CrateData, CrateVersionData } from '../types/crate_data';

defineProps({
  crate: {
    type: Object as () => CrateData,
    required: true
  },
  selectedVersion: {
    type: Object as () => CrateVersionData,
    required: true
  },
  flattenedFeatures: {
    type: Array as () => string[],
    required: true
  },
  sortedOwners: {
    type: Array as () => string[],
    required: true
  }
});
</script>

<style scoped>
.about-tab-container {
  margin-top: 8px;
}

.card-header {
  background-color: var(--v-theme-surface-variant);
  color: var(--v-theme-on-surface-variant);
  border-bottom: 1px solid var(--v-theme-outline-variant);
  padding: 12px 16px;
}

.card-surface {
  background-color: var(--v-theme-surface);
  transition: transform 0.2s, box-shadow 0.2s;
}

.card-surface:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1) !important;
}

.info-grid {
  display: grid;
  grid-template-columns: minmax(100px, 120px) 1fr;
  row-gap: 12px;
  column-gap: 16px;
  align-items: center;
}

.info-label {
  font-weight: 500;
  color: var(--v-theme-on-surface-variant);
  display: flex;
  align-items: center;
}

.info-content {
  overflow: hidden;
  text-overflow: ellipsis;
  word-break: break-word;
}

.external-link {
  display: inline-flex;
  align-items: center;
  color: var(--v-theme-primary);
  transition: opacity 0.2s;
}

.external-link:hover {
  opacity: 0.8;
}

.chip-container {
  display: flex;
  flex-wrap: wrap;
  margin: -4px;
}

.features-container {
  display: flex;
  flex-wrap: wrap;
  margin: -4px;
  max-height: 300px;
  overflow-y: auto;
}

/* Dark mode adjustments */
:deep(.v-theme--dark) .card-surface:hover {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4) !important;
}

:deep(.v-theme--dark) .external-link {
  color: var(--v-theme-primary-lighten-1);
}

@media (max-width: 600px) {
  .info-grid {
    grid-template-columns: 1fr;
    row-gap: 8px;
  }

  .info-label {
    margin-bottom: 4px;
  }
}
</style>
