<template>
  <v-card class="crate-card mb-3" elevation="0" data-testid="crate-card" @click="navigateToCrate">
    <v-card-text class="pa-4">
      <div class="d-flex align-start">
        <!-- Origin Logo -->
        <div class="logo-container mr-4">
          <!-- A plain image shows at once, v-img fades in and leaves an empty circle meanwhile -->
          <v-avatar size="48" class="crate-logo">
            <img v-if="props.isCache" :src="store.cargoSmallLogo" alt="Crates.io logo" width="32" height="32" />
            <img v-else :src="store.kellnrSmallLogo" alt="Kellnr logo" width="32" height="32" />
          </v-avatar>
        </div>

        <!-- Main Content -->
        <div class="flex-grow-1 min-width-0">
          <!-- Header Row: Name + Version + Stats -->
          <div class="d-flex flex-wrap align-center justify-space-between mb-2">
            <div class="d-flex align-center flex-wrap crate-header">
              <span class="crate-name font-weight-bold me-3" data-testid="crate-card-name">{{ crate }}</span>
              <span class="version-label tabular-nums" data-testid="crate-card-version">v{{ version }}</span>
            </div>

            <!-- Stats Row -->
            <div class="d-flex align-center stats-row">
              <v-tooltip location="top" text="Downloads">
                <template v-slot:activator="{ props: tooltipProps }">
                  <div class="stat-item" v-bind="tooltipProps">
                    <v-icon icon="mdi-download" size="x-small" class="stat-icon" />
                    <span class="stat-value tabular-nums" data-testid="crate-card-downloads">{{ formatNumber(downloads) }}</span>
                  </div>
                </template>
              </v-tooltip>

              <v-tooltip location="top" text="Last updated">
                <template v-slot:activator="{ props: tooltipProps }">
                  <div class="stat-item" v-bind="tooltipProps">
                    <v-icon icon="mdi-calendar-outline" size="x-small" class="stat-icon" />
                    <span class="stat-value" data-testid="crate-card-updated" :title="exactDate(updated)">{{ humanizedLastUpdated }}</span>
                  </div>
                </template>
              </v-tooltip>

              <!-- Only crates with docs get a link, so it never leads nowhere -->
              <a v-if="isSafeUrl(docLink)" :href="docLink" class="stat-item doc-link" target="_blank"
                rel="noopener noreferrer" data-testid="crate-card-docs-link" @click.stop>
                <v-icon icon="mdi-file-document-outline" size="x-small" />
                <span>Docs</span>
              </a>
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
import { useStore } from "../store/store";
import { useRouter } from "vue-router";
import { isSafeUrl } from "../common/url";
import { formatNumber } from "../common/format";
import { useRelativeTime } from "../composables";

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

const { fromNow, exactDate } = useRelativeTime();

const humanizedLastUpdated = computed(() => fromNow(props.updated))

// Navigate to crate details or crates.io
function navigateToCrate() {
  if (props.isCache) {
    window.open(`https://crates.io/crates/${props.crate}`, '_blank', 'noopener,noreferrer');
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
  background: rgb(var(--v-theme-surface-variant));
}

.crate-header {
  gap: 8px;
}

.crate-name {
  font-size: 1.125rem;
  color: rgb(var(--v-theme-on-surface));
  word-break: break-word;
}

.version-label {
  font-size: 0.8125rem;
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface-variant));
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
  color: rgb(var(--v-theme-on-surface-variant));
}

.stat-value {
  white-space: nowrap;
  font-size: 0.9rem;
  color: rgb(var(--v-theme-on-surface));
  font-weight: 500;
}

.doc-link {
  font-size: 0.9rem;
  font-weight: 500;
  color: rgb(var(--v-theme-primary));
  text-decoration: none;
}

.doc-link:hover {
  text-decoration: underline;
}

/* Responsive adjustments */
@media (max-width: 768px) {
  .stats-row {
    width: 100%;
    margin-top: 8px;
    justify-content: flex-start;
    flex-wrap: wrap;
    row-gap: 8px;
  }

  .crate-header {
    width: 100%;
  }
}
</style>
