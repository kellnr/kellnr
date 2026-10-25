<template>
  <div class="version-item" data-testid="version-row" @click="openCrateVersionPage">
    <div class="version-main">
      <!-- Left: Version with tag icon -->
      <div class="version-info">
        <div class="version-header">
          <span class="version-number" data-testid="version-number">{{ version }}</span>
          <v-icon icon="mdi-tag-outline" size="small" class="version-tag-icon"></v-icon>
        </div>
      </div>

      <!-- Middle: Date -->
      <div class="version-date">
        <v-icon icon="mdi-calendar-outline" size="small" class="me-2"></v-icon>
        <span :title="exactDate(props.last_updated)">{{ humanizedLastUpdated }}</span>
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
import { formatNumber } from "../common/format";
import { useRelativeTime } from "../composables";

const props = defineProps<{
  name: string,
  version: string,
  last_updated: string,
  downloads: string
}>();

const router = useRouter();
const { fromNow, exactDate } = useRelativeTime();

const humanizedLastUpdated = computed(() => fromNow(props.last_updated));

const formattedDownloads = computed(() => formatNumber(props.downloads));

function openCrateVersionPage() {
  router.push({ name: 'Crate', query: { name: props.name, version: props.version } });
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
  flex-shrink: 0;
  min-width: 100px;
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

.version-tag-icon {
  color: rgb(var(--v-theme-info));
  opacity: 0.8;
}

.version-date {
  flex: 1;
  display: flex;
  align-items: center;
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

  .version-info {
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
