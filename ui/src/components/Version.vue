<template>
  <div class="version-item" :class="{ yanked: version.yanked }" data-testid="version-row" @click="openCrateVersionPage">
    <div class="version-main">
      <!-- Left: Version, with a marker if it is yanked -->
      <div class="version-info">
        <div class="version-header">
          <span class="version-number" data-testid="version-number">{{ version.version }}</span>
          <span v-if="version.yanked" class="yanked-badge" data-testid="version-yanked">Yanked</span>
        </div>

        <!-- Size, MSRV, license and features -->
        <div class="version-meta" data-testid="version-meta">
          <span class="meta-item" title="Size of the crate file" data-testid="version-size">
            <v-icon icon="mdi-package-variant-closed" size="x-small" />
            <span class="tabular-nums">{{ formattedSize }}</span>
          </span>
          <span v-if="version.rust_version" class="meta-item" title="Minimal supported Rust version"
            data-testid="version-msrv">
            <v-icon icon="mdi-language-rust" size="x-small" />
            <span class="tabular-nums">v{{ version.rust_version }}</span>
          </span>
          <span v-if="version.license" class="meta-item" title="License" data-testid="version-license">
            <v-icon icon="mdi-scale-balance" size="x-small" />
            <span>{{ version.license }}</span>
          </span>
          <span v-if="featureCount > 0" class="meta-item" title="Features" data-testid="version-features">
            <v-icon icon="mdi-widgets-outline" size="x-small" />
            <span>{{ featureCount }} {{ featureCount === 1 ? "feature" : "features" }}</span>
          </span>
        </div>
      </div>

      <!-- Middle: Date -->
      <div class="version-date">
        <v-icon icon="mdi-calendar-outline" size="small" class="me-2"></v-icon>
        <span :title="exactDate(version.created)">{{ humanizedLastUpdated }}</span>
        <span v-if="version.publisher" class="version-publisher" :title="publisherTitle"
          data-testid="version-publisher">&nbsp;by {{ version.publisher }}</span>
      </div>

      <!-- Right: Downloads -->
      <div class="version-downloads">
        <v-icon icon="mdi-download" size="small" class="me-2"></v-icon>
        <span>{{ formattedDownloads }}</span>
      </div>
    </div>

    <!-- Chevron indicator -->
    <v-icon icon="mdi-chevron-right" size="small" class="version-chevron"></v-icon>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import { formatBytes, formatNumber } from "../common/format";
import { useRelativeTime } from "../composables";
import type { CrateVersionData } from "../types/crate_data";

const props = defineProps<{
  name: string,
  version: CrateVersionData,
}>();

const router = useRouter();
const { fromNow, exactDate } = useRelativeTime();

const humanizedLastUpdated = computed(() => fromNow(props.version.created));

const formattedDownloads = computed(() => formatNumber(props.version.downloads));

// Zero for versions whose crate file could not be read when Kellnr started
// recording sizes
const formattedSize = computed(() => props.version.size > 0 ? formatBytes(props.version.size) : "Size unknown");

const publisherTitle = computed(() => props.version.publisher_token
  ? `Published with the token ${props.version.publisher_token}`
  : undefined);

const featureCount = computed(() => Object.keys(props.version.features).length);

function openCrateVersionPage() {
  router.push({ name: 'Crate', query: { name: props.name, version: props.version.version } });
}
</script>

<style scoped>
.version-item {
  display: flex;
  align-items: center;
  padding: 12px 16px;
  margin-bottom: 8px;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--kellnr-radius);
  cursor: pointer;
  transition: all 0.2s ease;
}

.version-item:hover {
  background: rgb(var(--v-theme-surface));
  border-color: rgb(var(--v-theme-primary));
}

.version-item:last-child {
  margin-bottom: 0;
}

.version-main {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 24px;
  min-width: 0;
}

.version-info {
  flex: 1;
  min-width: 0;
}

.version-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.version-number {
  font-weight: 600;
  font-size: 16px;
  font-family: var(--kellnr-font-mono);
  color: rgb(var(--v-theme-primary));
}

/* A yanked version is still listed, but struck through */
.yanked .version-number {
  text-decoration: line-through;
  color: rgb(var(--v-theme-on-surface-variant));
}

.yanked-badge {
  padding: 1px 8px;
  border-radius: var(--kellnr-radius);
  font-size: 12px;
  font-weight: 500;
  background: rgba(var(--v-theme-error), 0.12);
  color: rgb(var(--v-theme-error));
}

.version-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 16px;
  margin-top: 4px;
  font-size: 13px;
  color: rgb(var(--v-theme-on-surface-variant));
}

.meta-item {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
}

.version-date {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  min-width: 150px;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface-variant));
}

.version-date .v-icon {
  color: rgb(var(--v-theme-info));
  opacity: 0.8;
}

.version-downloads {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  font-size: 15px;
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface-variant));
  min-width: 70px;
  justify-content: flex-end;
}

.version-downloads .v-icon {
  color: rgb(var(--v-theme-success));
  opacity: 0.8;
}

.version-chevron {
  flex-shrink: 0;
  color: rgb(var(--v-theme-on-surface-variant));
  margin-left: 12px;
  opacity: 0.5;
  transition: all 0.2s ease;
}

.version-item:hover .version-chevron {
  opacity: 1;
  color: rgb(var(--v-theme-primary));
  transform: translateX(2px);
}

/* Responsive adjustments */
@media (max-width: 768px) {
  .version-main {
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }

  .version-date {
    min-width: auto;
  }

  .version-downloads {
    min-width: auto;
    justify-content: flex-start;
  }
}

@media (max-width: 480px) {
  .version-item {
    padding: 10px 12px;
  }
}
</style>
