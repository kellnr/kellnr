<template>
  <div class="about-content">
    <!-- Basic Information Section. Repository, homepage and license are in the sidebar. -->
    <div v-if="selectedVersion.yanked === true" class="basic-info-section">
      <!-- Status (Yanked) -->
      <div v-if="selectedVersion.yanked === true" class="info-row">
        <div class="info-icon-wrapper error">
          <v-icon icon="mdi-alert-circle" size="small"></v-icon>
        </div>
        <span class="info-label">Status</span>
        <span class="info-badge error-badge">Yanked</span>
      </div>
    </div>

    <!-- Main Content Grid -->
    <div class="metadata-grid">
      <!-- Authors Section -->
      <div v-if="crate.authors && crate.authors.length > 0" class="section-card" data-testid="about-authors">
        <div class="section-header">
          <v-icon icon="mdi-account-multiple" size="small" class="section-icon"></v-icon>
          <span class="section-title">Authors</span>
          <span class="count-badge">{{ crate.authors.length }}</span>
        </div>
        <div class="section-content">
          <span v-for="(author, i) in crate.authors" :key="`author-${i}`" class="item-chip" data-testid="about-author">
            {{ author }}
          </span>
        </div>
      </div>

      <!-- Owners Section -->
      <div v-if="sortedOwners.length > 0" class="section-card" data-testid="about-owners">
        <div class="section-header">
          <v-icon icon="mdi-shield-account" size="small" class="section-icon"></v-icon>
          <span class="section-title">Owners</span>
          <span class="count-badge">{{ sortedOwners.length }}</span>
        </div>
        <div class="section-content">
          <span v-for="(owner, i) in sortedOwners" :key="`owner-${i}`" class="item-chip" data-testid="about-owner">
            {{ owner }}
          </span>
        </div>
      </div>

      <!-- Categories Section -->
      <div v-if="crate.categories && crate.categories.length > 0" class="section-card" data-testid="about-categories">
        <div class="section-header">
          <v-icon icon="mdi-folder-multiple" size="small" class="section-icon"></v-icon>
          <span class="section-title">Categories</span>
          <span class="count-badge">{{ crate.categories.length }}</span>
        </div>
        <div class="section-content">
          <span v-for="(category, i) in crate.categories" :key="`category-${i}`" class="item-chip"
            data-testid="about-category">
            {{ category }}
          </span>
        </div>
      </div>

      <!-- Keywords Section -->
      <div v-if="crate.keywords && crate.keywords.length > 0" class="section-card" data-testid="about-keywords">
        <div class="section-header">
          <v-icon icon="mdi-tag-multiple" size="small" class="section-icon"></v-icon>
          <span class="section-title">Keywords</span>
          <span class="count-badge">{{ crate.keywords.length }}</span>
        </div>
        <div class="section-content">
          <span v-for="(keyword, i) in crate.keywords" :key="`keyword-${i}`" class="item-chip"
            data-testid="about-keyword">
            {{ keyword }}
          </span>
        </div>
      </div>

      <!-- Features Section -->
      <div v-if="flattenedFeatures.length > 0" class="section-card features-section">
        <div class="section-header">
          <v-icon icon="mdi-widgets" size="small" class="section-icon"></v-icon>
          <span class="section-title">Features</span>
          <span class="count-badge">{{ flattenedFeatures.length }}</span>
        </div>
        <div class="section-content">
          <span v-for="(feature, i) in flattenedFeatures" :key="`feature-${i}`" class="item-chip feature-chip">
            {{ feature }}
          </span>
        </div>
      </div>
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
  padding: 24px;
  color: rgb(var(--v-theme-on-surface));
}

/* Basic Information Section */
.basic-info-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  margin-bottom: 24px;
  background: rgb(var(--v-theme-surface-variant));
  border-radius: var(--kellnr-radius);
  border: 1px solid rgb(var(--v-theme-outline));
}

.info-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.info-icon-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--kellnr-radius);
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-primary));
}

.info-icon-wrapper.error {
  color: rgb(var(--v-theme-error));
}

.info-label {
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface-variant));
  min-width: 80px;
}

.info-badge {
  display: inline-block;
  padding: 4px 10px;
  border-radius: var(--kellnr-radius);
  font-size: 13px;
  font-weight: 500;
}

.error-badge {
  background: rgba(var(--v-theme-error), 0.12);
  color: rgb(var(--v-theme-error));
}

/* Metadata Grid */
.metadata-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 16px;
}

.section-card {
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--kellnr-radius);
  padding: 16px;
}

.section-header {
  display: flex;
  align-items: center;
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.section-icon {
  color: rgb(var(--v-theme-on-surface-variant));
  margin-right: 8px;
}

.section-title {
  font-weight: 600;
  font-size: 14px;
  color: rgb(var(--v-theme-on-surface));
}

.count-badge {
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface-variant));
  font-size: 11px;
  font-weight: 500;
  padding: 2px 8px;
  border-radius: 9999px;
  margin-left: auto;
}

.section-content {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.item-chip {
  display: inline-block;
  padding: 4px 10px;
  border-radius: var(--kellnr-radius);
  font-size: 13px;
  line-height: 1.4;
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  transition: all 0.2s ease;
}

.item-chip:hover {
  border-color: rgb(var(--v-theme-primary));
}

.feature-chip {
  font-family: var(--kellnr-font-mono);
  font-size: 12px;
}

.features-section {
  grid-column: 1 / -1;
}

/* Responsive */
@media (max-width: 600px) {
  .about-content {
    padding: 16px;
  }

  .basic-info-section {
    padding: 12px;
  }

  .metadata-grid {
    grid-template-columns: 1fr;
  }

  .info-label {
    min-width: 70px;
  }
}
</style>
