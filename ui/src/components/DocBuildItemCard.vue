<template>
  <div class="build-item" :class="{ 'last-item': isLast }" data-testid="doc-build-item">
    <div class="build-item-content">
      <div class="crate-info">
        <router-link class="crate-link" :to="{ name: 'Crate', query: { name: build.name, version: build.version } }">
          {{ build.name }}
        </router-link>
      </div>
      <div class="version-info">
        <v-chip size="small" variant="tonal" color="primary" class="version-chip">
          v{{ build.version }}
        </v-chip>
      </div>
      <div class="status-info">
        <v-chip size="small" variant="tonal" :color="status.color" :prepend-icon="status.icon"
          data-testid="doc-build-status">
          {{ status.label }}
        </v-chip>
      </div>
      <div class="time-info" :title="exactDate(build.finished)">
        {{ fromNow(build.finished) }} &middot; {{ duration }}
      </div>
      <div class="actions">
        <v-btn v-if="build.error" size="small" variant="text"
          :append-icon="showError ? 'mdi-chevron-up' : 'mdi-chevron-down'" data-testid="doc-build-toggle-error"
          @click="showError = !showError">
          Output
        </v-btn>
        <v-btn v-if="canRebuild" size="small" variant="outlined" color="primary" prepend-icon="mdi-refresh"
          :loading="rebuilding" data-testid="doc-build-rebuild" @click="emit('rebuild', build)">
          Rebuild
        </v-btn>
      </div>
    </div>
    <v-expand-transition>
      <pre v-if="showError && build.error" class="build-error" data-testid="doc-build-error">{{ build.error }}</pre>
    </v-expand-transition>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue"
import type { DocBuild, DocBuildStatus } from "../types/doc_build"
import { useRelativeTime } from "../composables/useRelativeTime"

const props = defineProps<{
  build: DocBuild
  isLast?: boolean
  rebuilding?: boolean
  /** The version is in the queue again or has a newer build */
  queued?: boolean
}>()

const emit = defineEmits<{
  rebuild: [build: DocBuild]
}>()

const { fromNow, exactDate } = useRelativeTime()
const showError = ref(false)

const STATUS: Record<DocBuildStatus, { label: string; color: string; icon: string }> = {
  succeeded: { label: "Succeeded", color: "success", icon: "mdi-check-circle-outline" },
  failed: { label: "Failed", color: "error", icon: "mdi-alert-circle-outline" },
  timed_out: { label: "Timed out", color: "warning", icon: "mdi-timer-alert-outline" },
}

const status = computed(() => STATUS[props.build.status])

// Successful and queued builds need no rebuild from this page
const canRebuild = computed(
  () => props.build.can_rebuild && props.build.status !== "succeeded" && !props.queued
)

const duration = computed(() => {
  const millis = Date.parse(props.build.finished) - Date.parse(props.build.started)
  const seconds = Math.max(0, Math.round(millis / 1000))
  const minutes = Math.floor(seconds / 60)
  return minutes > 0 ? `${minutes} min ${seconds % 60} s` : `${seconds} s`
})
</script>

<style scoped>
.build-item {
  padding: 16px 20px;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
  transition: background-color 0.2s ease;
}

.build-item:hover {
  background: rgba(var(--v-theme-primary), 0.03);
}

.build-item.last-item {
  border-bottom: none;
}

.build-item-content {
  display: grid;
  grid-template-columns: 1fr auto auto auto auto;
  align-items: center;
  gap: 16px;
}

.crate-info {
  min-width: 0;
}

.crate-link {
  font-size: 1rem;
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface));
  text-decoration: none;
  transition: color 0.2s ease;
}

.crate-link:hover {
  color: rgb(var(--v-theme-primary));
  text-decoration: underline;
}

.version-chip {
  font-size: 0.75rem;
  font-weight: 500;
}

.time-info {
  font-size: 0.8rem;
  color: rgb(var(--v-theme-on-surface-variant));
  white-space: nowrap;
}

.actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.build-error {
  margin: 12px 0 0;
  padding: 12px;
  max-height: 320px;
  overflow: auto;
  font-family: var(--kellnr-font-mono);
  font-size: 0.75rem;
  white-space: pre-wrap;
  word-break: break-word;
  background: rgba(var(--v-theme-on-surface), 0.04);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--kellnr-radius);
}

/* Responsive adjustments */
@media (max-width: 600px) {
  .build-item-content {
    grid-template-columns: 1fr auto;
    gap: 8px 12px;
  }

  .time-info,
  .actions {
    grid-column: 1 / -1;
    justify-content: flex-start;
  }
}
</style>
