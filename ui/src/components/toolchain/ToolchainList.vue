<template>
  <div v-if="toolchains.length > 0" class="toolchains-section mb-6">
    <SubsectionHeader icon="mdi-package-variant" title="Available Toolchains" />

    <div class="toolchain-list">
      <v-expansion-panels variant="accordion" class="toolchain-panels">
        <v-expansion-panel v-for="toolchain in toolchains" :key="`${toolchain.name}-${toolchain.version}`">
          <v-expansion-panel-title class="panel-title">
            <div class="toolchain-info">
              <div class="toolchain-avatar">
                <v-icon icon="mdi-package-variant-closed" size="small"></v-icon>
              </div>
              <div class="toolchain-details">
                <span class="toolchain-name">{{ toolchain.name }} {{ toolchain.version }}</span>
                <span class="toolchain-date">{{ toolchain.date }} · {{ toolchain.targets.length }} {{ toolchain.targets.length === 1 ? 'target' : 'targets' }}</span>
              </div>
              <v-chip
                v-if="toolchain.channel"
                size="x-small"
                :color="getChannelColor(toolchain.channel)"
                variant="tonal"
                class="ms-2"
              >
                {{ toolchain.channel }}
              </v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <!-- Channel Assignment -->
            <div class="channel-assignment mb-4">
              <div class="channel-row">
                <div class="channel-label">
                  <v-icon icon="mdi-source-branch" size="x-small" class="me-2 text-medium-emphasis"></v-icon>
                  <span class="text-body-2">Release Channel</span>
                </div>
                <div class="channel-controls">
                  <v-select
                    v-model="toolchain.channel"
                    :items="channelOptions"
                    density="compact"
                    variant="outlined"
                    hide-details
                    clearable
                    placeholder="None"
                    class="channel-select"
                    @update:model-value="$emit('channel-change', toolchain)"
                  ></v-select>
                </div>
              </div>
              <p class="text-caption text-medium-emphasis mt-1 mb-0">
                Assign this toolchain to a channel so users can install it via <code>rustup install {{ toolchain.channel || 'channel-name' }}</code>
              </p>
            </div>

            <!-- Targets List -->
            <div class="targets-section">
              <div class="targets-header">
                <span class="text-body-2 font-weight-medium">Targets</span>
                <v-btn
                  color="error"
                  variant="text"
                  size="small"
                  @click.stop="$emit('delete-toolchain', toolchain.name, toolchain.version)"
                >
                  <v-icon icon="mdi-delete-outline" size="small" class="me-1"></v-icon>
                  Delete All
                </v-btn>
              </div>
              <div class="targets-list">
                <ListItem
                  v-for="target in toolchain.targets"
                  :key="target.target"
                  icon="mdi-chip"
                  compact
                  test-id="target-item"
                >
                  <template #title>
                    <span class="target-name">{{ target.target }}</span>
                    <span class="target-size">{{ formatSize(target.size) }}</span>
                  </template>
                  <template #actions>
                    <v-btn
                      color="error"
                      variant="text"
                      size="small"
                      @click.stop="$emit('delete-target', toolchain.name, toolchain.version, target.target)"
                    >
                      <v-icon icon="mdi-delete-outline" size="small"></v-icon>
                    </v-btn>
                  </template>
                </ListItem>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>
    </div>
  </div>

  <EmptyState
    v-else
    icon="mdi-hammer-wrench"
    message="No toolchains uploaded yet."
    class="mb-6"
  />
</template>

<script setup lang="ts">
import type { Toolchain } from "../../types/toolchain"
import { SubsectionHeader, ListItem, EmptyState } from "../common"

defineProps<{
  toolchains: Toolchain[]
  channelOptions: string[]
}>()

defineEmits<{
  (e: 'channel-change', toolchain: Toolchain): void
  (e: 'delete-toolchain', name: string, version: string): void
  (e: 'delete-target', name: string, version: string, target: string): void
}>()

function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B"
  const k = 1024
  const sizes = ["B", "KB", "MB", "GB"]
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i]
}

function getChannelColor(channel: string): string {
  switch (channel) {
    case "stable": return "success"
    case "beta": return "warning"
    case "nightly": return "info"
    default: return "primary"
  }
}
</script>

<style scoped>
/* Toolchain Panels */
.toolchain-panels {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.toolchain-panels :deep(.v-expansion-panel) {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px !important;
  margin-bottom: 0;
}

.toolchain-panels :deep(.v-expansion-panel::before) {
  box-shadow: none;
}

.toolchain-panels :deep(.v-expansion-panel--active) {
  border-color: rgb(var(--v-theme-primary));
}

.panel-title {
  min-height: 56px;
}

.toolchain-info {
  display: flex;
  align-items: center;
  width: 100%;
}

.toolchain-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  background: rgba(var(--v-theme-primary), 0.15);
  color: rgb(var(--v-theme-primary));
  margin-right: 12px;
}

.toolchain-details {
  display: flex;
  flex-direction: column;
}

.toolchain-name {
  font-weight: 600;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.toolchain-date {
  font-size: 12px;
  color: rgb(var(--v-theme-on-surface-variant));
}

/* Channel Assignment */
.channel-assignment {
  padding: 12px 16px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
}

.channel-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.channel-label {
  display: flex;
  align-items: center;
}

.channel-controls {
  flex: 0 0 180px;
}

.channel-select {
  font-size: 14px;
}

.channel-select :deep(.v-field) {
  border-radius: 6px;
}

/* Targets Section */
.targets-section {
  margin-top: 8px;
}

.targets-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.targets-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.target-name {
  font-family: 'Roboto Mono', monospace;
  font-size: 13px;
  color: rgb(var(--v-theme-on-surface));
}

.target-size {
  font-size: 12px;
  color: rgb(var(--v-theme-on-surface-variant));
  background: rgba(var(--v-theme-primary), 0.08);
  padding: 2px 8px;
  border-radius: 4px;
  margin-left: 8px;
}

/* Responsive */
@media (max-width: 600px) {
  .channel-row {
    flex-direction: column;
    align-items: stretch;
  }

  .channel-controls {
    flex: 1;
  }
}
</style>
