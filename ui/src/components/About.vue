<template>
  <div class="about-content pa-4">
    <!-- Basic Information Section -->
    <div class="basic-info mb-5">
      <div class="d-flex flex-wrap">
        <!-- Repository -->
        <div v-if="crate.repository" class="info-item">
          <div class="d-flex align-center">
            <v-icon icon="mdi-github" color="grey-darken-1" size="small" class="me-2"></v-icon>
            <span class="info-label">Repository:</span>
            <a :href="crate.repository" target="_blank" class="external-link ms-2">
              {{ crate.repository }}
              <v-icon icon="mdi-open-in-new" size="x-small" class="ms-1"></v-icon>
            </a>
          </div>
        </div>

        <!-- Homepage -->
        <div v-if="crate.homepage" class="info-item">
          <div class="d-flex align-center">
            <v-icon icon="mdi-link-variant" color="blue" size="small" class="me-2"></v-icon>
            <span class="info-label">Homepage:</span>
            <a :href="crate.homepage" target="_blank" class="external-link ms-2">
              {{ crate.homepage }}
              <v-icon icon="mdi-open-in-new" size="x-small" class="ms-1"></v-icon>
            </a>
          </div>
        </div>

        <!-- License -->
        <div v-if="selectedVersion.license" class="info-item">
          <div class="d-flex align-center">
            <v-icon icon="mdi-scale-balance" color="indigo" size="small" class="me-2"></v-icon>
            <span class="info-label">License:</span>
            <v-chip size="x-small" color="indigo" variant="flat" class="ms-2">
              {{ selectedVersion.license }}
            </v-chip>
          </div>
        </div>

        <!-- Status (Yanked) -->
        <div v-if="selectedVersion.yanked === true" class="info-item">
          <div class="d-flex align-center">
            <v-icon icon="mdi-alert-circle" color="error" size="small" class="me-2"></v-icon>
            <span class="info-label">Status:</span>
            <v-chip size="x-small" color="error" variant="flat" class="ms-2">Yanked</v-chip>
          </div>
        </div>
      </div>
    </div>

    <!-- Main Content Grid -->
    <div class="metadata-grid">
      <!-- Authors Section -->
      <div v-if="crate.authors && crate.authors.length > 0" class="section-container">
        <div class="section-header">
          <v-icon icon="mdi-account-multiple" color="purple" class="me-2"></v-icon>
          <span class="text-subtitle-1 font-weight-medium">Authors</span>
          <span class="count-badge">{{ crate.authors.length }}</span>
        </div>

        <div class="section-content">
          <span v-for="(author, i) in crate.authors" :key="`author-${i}`" class="compact-chip purple">
            {{ author }}
          </span>
        </div>
      </div>

      <!-- Owners Section -->
      <div v-if="sortedOwners.length > 0" class="section-container">
        <div class="section-header">
          <v-icon icon="mdi-shield-account" color="deep-orange" class="me-2"></v-icon>
          <span class="text-subtitle-1 font-weight-medium">Owners</span>
          <span class="count-badge">{{ sortedOwners.length }}</span>
        </div>

        <div class="section-content">
          <span v-for="(owner, i) in sortedOwners" :key="`owner-${i}`" class="compact-chip deep-orange"
            :class="{ 'current-user': owner === 'secana' }">
            {{ owner }}
          </span>
        </div>
      </div>

      <!-- Categories Section -->
      <div v-if="crate.categories && crate.categories.length > 0" class="section-container">
        <div class="section-header">
          <v-icon icon="mdi-folder-multiple" color="teal" class="me-2"></v-icon>
          <span class="text-subtitle-1 font-weight-medium">Categories</span>
          <span class="count-badge">{{ crate.categories.length }}</span>
        </div>

        <div class="section-content">
          <span v-for="(category, i) in crate.categories" :key="`category-${i}`" class="compact-chip teal">
            {{ category }}
          </span>
        </div>
      </div>

      <!-- Keywords Section -->
      <div v-if="crate.keywords && crate.keywords.length > 0" class="section-container">
        <div class="section-header">
          <v-icon icon="mdi-tag-multiple" color="green" class="me-2"></v-icon>
          <span class="text-subtitle-1 font-weight-medium">Keywords</span>
          <span class="count-badge">{{ crate.keywords.length }}</span>
        </div>

        <div class="section-content">
          <span v-for="(keyword, i) in crate.keywords" :key="`keyword-${i}`" class="compact-chip green">
            {{ keyword }}
          </span>
        </div>
      </div>

      <!-- Features Section -->
      <div v-if="flattenedFeatures.length > 0" class="section-container features-section">
        <div class="section-header">
          <v-icon icon="mdi-widgets" color="amber-darken-2" class="me-2"></v-icon>
          <span class="text-subtitle-1 font-weight-medium">Features</span>
          <span class="count-badge">{{ flattenedFeatures.length }}</span>
        </div>

        <div class="section-content">
          <span v-for="(feature, i) in flattenedFeatures" :key="`feature-${i}`" class="compact-chip amber">
            {{ feature }}
          </span>
        </div>
      </div>
    </div>

    <!-- Updated info -->
    <div class="d-flex justify-end text-caption text-medium-emphasis mt-3">
      <v-icon icon="mdi-clock-outline" size="x-small" class="me-1"></v-icon>
      <span>Updated: 2025-04-16 11:02:27 UTC</span>
    </div>
  </div>
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
.about-content {
  background-color: var(--v-theme-surface);
  border-radius: 4px;
}

.info-item {
  padding: 4px 16px 4px 0;
  margin-right: 16px;
  margin-bottom: 4px;
  white-space: nowrap;
}

.info-label {
  font-weight: 500;
  color: var(--v-theme-on-surface-variant);
  margin-right: 4px;
}

.external-link {
  display: inline-flex;
  align-items: center;
  color: var(--v-theme-primary);
  text-decoration: none;
  transition: opacity 0.2s;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 240px;
}

.external-link:hover {
  opacity: 0.8;
  text-decoration: underline;
}

.metadata-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 16px;
}

.section-container {
  margin-bottom: 8px;
}

.section-header {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--v-theme-outline-variant);
}

.count-badge {
  background-color: var(--v-theme-surface-variant);
  color: var(--v-theme-on-surface-variant);
  font-size: 12px;
  padding: 0 6px;
  border-radius: 10px;
  margin-left: 8px;
  min-width: 20px;
  text-align: center;
}

.section-content {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.compact-chip {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 12px;
  font-size: 12px;
  line-height: 20px;
  white-space: nowrap;
}

.compact-chip.purple {
  background-color: rgba(156, 39, 176, 0.12);
  color: rgb(123, 31, 162);
}

.compact-chip.deep-orange {
  background-color: rgba(230, 74, 25, 0.12);
  color: rgb(191, 54, 12);
}

.compact-chip.teal {
  background-color: rgba(0, 150, 136, 0.12);
  color: rgb(0, 121, 107);
}

.compact-chip.green {
  background-color: rgba(76, 175, 80, 0.12);
  color: rgb(46, 125, 50);
}

.compact-chip.amber {
  background-color: rgba(255, 193, 7, 0.12);
  color: rgb(255, 111, 0);
}

.current-user {
  background-color: rgba(230, 74, 25, 0.2);
  font-weight: 500;
  border: 1px solid rgba(230, 74, 25, 0.3);
}

.features-section {
  grid-column: 1 / -1;
}

/* Dark mode adjustments */
:deep(.v-theme--dark) .compact-chip.purple {
  background-color: rgba(156, 39, 176, 0.15);
  color: rgb(186, 104, 200);
}

:deep(.v-theme--dark) .compact-chip.deep-orange {
  background-color: rgba(230, 74, 25, 0.15);
  color: rgb(255, 112, 67);
}

:deep(.v-theme--dark) .compact-chip.teal {
  background-color: rgba(0, 150, 136, 0.15);
  color: rgb(77, 182, 172);
}

:deep(.v-theme--dark) .compact-chip.green {
  background-color: rgba(76, 175, 80, 0.15);
  color: rgb(129, 199, 132);
}

:deep(.v-theme--dark) .compact-chip.amber {
  background-color: rgba(255, 193, 7, 0.15);
  color: rgb(255, 202, 40);
}

:deep(.v-theme--dark) .current-user {
  background-color: rgba(255, 112, 67, 0.25);
  border: 1px solid rgba(255, 112, 67, 0.4);
}

@media (max-width: 600px) {
  .metadata-grid {
    grid-template-columns: 1fr;
  }

  .external-link {
    max-width: 200px;
  }

  .info-item {
    padding: 4px 8px 4px 0;
    margin-right: 8px;
  }
}
</style>
