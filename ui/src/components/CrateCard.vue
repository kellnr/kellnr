<template>
  <v-card class="crate-card mb-3" elevation="0" rounded="lg" @click="navigateToCrate">
    <v-card-text class="pa-4">
      <div class="d-flex align-start">
        <!-- Origin Logo -->
        <div class="logo-container mr-4">
          <v-avatar size="48" class="crate-logo">
            <v-img v-if="props.isCache" :src="store.cargoSmallLogo" alt="Crates.io logo" />
            <v-img v-else :src="store.kellnrSmallLogo" alt="Kellnr logo" />
          </v-avatar>
        </div>

        <!-- Main Content -->
        <div class="flex-grow-1 min-width-0">
          <!-- Header Row: Name + Version + Stats -->
          <div class="d-flex flex-wrap align-center justify-space-between mb-2">
            <div class="d-flex align-center flex-wrap crate-header">
              <span class="crate-name font-weight-bold me-3">{{ crate }}</span>
              <v-chip size="small" variant="tonal" color="primary" class="version-chip">
                v{{ version }}
              </v-chip>
            </div>

            <!-- Stats Row -->
            <div class="d-flex align-center stats-row">
              <v-tooltip location="top" text="Downloads">
                <template v-slot:activator="{ props: tooltipProps }">
                  <div class="stat-item" v-bind="tooltipProps">
                    <v-icon icon="mdi-download" size="x-small" class="stat-icon" />
                    <span class="stat-value">{{ formatNumber(downloads) }}</span>
                  </div>
                </template>
              </v-tooltip>

              <v-tooltip location="top" text="Last updated">
                <template v-slot:activator="{ props: tooltipProps }">
                  <div class="stat-item" v-bind="tooltipProps">
                    <v-icon icon="mdi-calendar-outline" size="x-small" class="stat-icon" />
                    <span class="stat-value">{{ humanizedLastUpdated }}</span>
                  </div>
                </template>
              </v-tooltip>

              <div class="doc-link-wrapper">
                <a v-if="docLink && docLink.length > 0" :href="docLink" class="doc-button" target="_blank" @click.stop>
                  <v-icon icon="mdi-file-document-outline" size="small" />
                  <span>Documentation</span>
                </a>
                <button v-else class="doc-button" @click.stop="goToPublishDocs">
                  <v-icon icon="mdi-file-document-outline" size="small" />
                  <span>Documentation</span>
                </button>
              </div>
            </div>
          </div>

          <!-- Description -->
          <p class="crate-description mb-0">
            {{ desc || "No description available" }}
          </p>
        </div>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from "vue";
import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";
import utc from "dayjs/plugin/utc";
import { useStore } from "../store/store";
import { useRouter } from "vue-router";

dayjs.extend(relativeTime);
dayjs.extend(utc);
const store = useStore();
const router = useRouter();

const props = defineProps<{
  crate: string
  desc?: string
  downloads: number
  version: string
  updated: string
  docLink?: string
  isCache: boolean
}>()

const humanizedLastUpdated = computed(() => {
  return dayjs.utc(props.updated).fromNow();
})

// Format number with commas for readability
function formatNumber(num: number): string {
  return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

// Navigate to crate details or crates.io
function navigateToCrate() {
  if (props.isCache) {
    window.open(`https://crates.io/crates/${props.crate}`, '_blank');
  } else {
    router.push({
      name: 'Crate',
      query: {
        name: props.crate,
        version: props.version
      }
    });
  }
}

// Navigate to publish docs page
function goToPublishDocs() {
  router.push({ name: 'PublishDocs' });
}
</script>

<style scoped>
.crate-card {
  cursor: pointer;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  transition: all 0.2s ease;
}

.crate-card:hover {
  background: rgba(var(--v-theme-primary), 0.04);
  border-color: rgb(var(--v-theme-primary));
}

.min-width-0 {
  min-width: 0;
}

.logo-container {
  flex-shrink: 0;
}

.crate-logo {
  background: rgba(var(--v-theme-primary), 0.1);
}

.crate-header {
  gap: 8px;
}

.crate-name {
  font-size: 1.125rem;
  color: rgb(var(--v-theme-on-surface));
  word-break: break-word;
}

.version-chip {
  font-size: 0.75rem;
  font-weight: 500;
  height: 22px;
}

.crate-description {
  font-size: 0.875rem;
  line-height: 1.5;
  color: rgb(var(--v-theme-on-surface-variant));
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.stats-row {
  gap: 20px;
  flex-shrink: 0;
}

.stat-item {
  display: flex;
  align-items: center;
  gap: 6px;
}

.stat-icon {
  color: rgb(var(--v-theme-primary));
  opacity: 0.7;
}

.stat-value {
  font-size: 0.9rem;
  color: rgb(var(--v-theme-on-surface));
  font-weight: 500;
}

/* Documentation Button */
.doc-link-wrapper {
  margin-left: 4px;
}

.doc-button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: rgba(var(--v-theme-primary), 0.1);
  color: rgb(var(--v-theme-primary));
  border: none;
  border-radius: 6px;
  font-size: 0.875rem;
  font-weight: 500;
  text-decoration: none;
  cursor: pointer;
  transition: all 0.2s ease;
}

.doc-button:hover {
  background: rgba(var(--v-theme-primary), 0.2);
}

/* Responsive adjustments */
@media (max-width: 768px) {
  .stats-row {
    width: 100%;
    margin-top: 8px;
    justify-content: flex-start;
  }

  .crate-header {
    width: 100%;
  }

  .doc-button {
    padding: 4px 10px;
    font-size: 0.8rem;
  }
}
</style>
