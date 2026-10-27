<template>
  <v-card class="mb-4 content-card" elevation="0" data-testid="crate-dependents">
    <SectionHeader icon="mdi-source-branch" title="Used by" :count="total" />
    <v-card-text>
      <div v-if="loading" class="loading-state">
        <v-progress-circular indeterminate size="18" width="2" color="primary" class="me-2" />
        Loading dependents...
      </div>
      <EmptyState v-else-if="failed" icon="mdi-alert-circle-outline" message="Cannot load the dependents." compact />
      <template v-else-if="data">
        <p class="dependents-summary" data-testid="dependents-summary">{{ dependentsSummary(data) }}</p>

        <router-link v-for="dependent in sortedDependents" :key="dependent.name"
          :to="{ name: 'Crate', query: { name: dependent.name } }" class="dependent-item"
          data-testid="dependent-row">
          <div class="dependent-header">
            <span class="dependent-name" data-testid="dependent-name">{{ dependent.name }}</span>
            <span class="dependent-version">{{ dependent.version }}</span>
          </div>
          <div class="dependent-requirements">
            <span v-for="req in dependent.requirements" :key="`${req.kind}-${req.version_req}`"
              class="requirement" :class="{ behind: req.outdated }" data-testid="dependent-requirement"
              :title="req.outdated ? `The latest version ${data.latest_version} is newer than ${req.version_req} allows` : undefined">
              <v-icon v-if="req.outdated" icon="mdi-arrow-up" size="x-small" />
              <span class="requirement-req">{{ req.version_req }}</span>
              <span v-if="req.kind !== 'normal'" class="requirement-kind">{{ req.kind }}</span>
            </span>
          </div>
        </router-link>

        <!-- Restricted dependents are only counted, nothing else is known about them -->
        <div v-for="n in data.restricted" :key="`restricted-${n}`" class="dependent-item restricted"
          data-testid="dependent-restricted">
          <div class="dependent-header">
            <v-icon icon="mdi-lock-outline" size="small" />
            <span class="dependent-name">Restricted</span>
          </div>
          <span class="restricted-hint">You have no access to this crate.</span>
        </div>
      </template>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { EmptyState, SectionHeader } from "../common";
import { crateService } from "../../services";
import { isSuccess } from "../../services/api";
import { dependentsSummary, sortDependents } from "../../common/dependents";
import type { CrateDependents } from "../../types/crate_data";

const props = defineProps<{
  crateName: string
}>()

const data = ref<CrateDependents | null>(null)
const loading = ref(true)
const failed = ref(false)

const total = computed(() => data.value ? data.value.dependents.length + data.value.restricted : undefined)
const sortedDependents = computed(() => data.value ? sortDependents(data.value.dependents) : [])

async function load() {
  loading.value = true
  failed.value = false
  const result = await crateService.getCrateDependents(props.crateName)
  if (isSuccess(result)) {
    data.value = result.data
  } else {
    console.error('Failed to load dependents:', result.error)
    data.value = null
    failed.value = true
  }
  loading.value = false
}

watch(() => props.crateName, load, { immediate: true })
</script>

<style scoped>
.loading-state {
  display: flex;
  align-items: center;
  color: rgb(var(--v-theme-on-surface-variant));
}

.dependents-summary {
  margin-bottom: 16px;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.dependent-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 8px 16px;
  padding: 12px 16px;
  margin-bottom: 8px;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--kellnr-radius);
  color: inherit;
  text-decoration: none;
  transition: all 0.2s ease;
}

.dependent-item:last-child {
  margin-bottom: 0;
}

a.dependent-item:hover,
a.dependent-item:focus-visible {
  background: rgb(var(--v-theme-surface));
  border-color: rgb(var(--v-theme-primary));
}

.dependent-header {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.dependent-name {
  font-weight: 600;
  font-size: 16px;
  color: rgb(var(--v-theme-on-surface));
  word-break: break-word;
}

.dependent-version {
  font-size: 15px;
  font-weight: 500;
  color: rgb(var(--v-theme-primary));
  font-family: var(--kellnr-font-mono);
}

.dependent-requirements {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.requirement {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 1px 8px;
  border-radius: 9999px;
  font-size: 13px;
  color: rgb(var(--v-theme-on-surface));
  border: 1px solid rgb(var(--v-theme-outline));
}

.requirement-req {
  font-family: var(--kellnr-font-mono);
}

.requirement-kind {
  color: rgb(var(--v-theme-on-surface-variant));
}

/* The warning color is too light for text in the light theme */
.requirement.behind {
  background: rgba(var(--v-theme-warning), 0.12);
  border-color: rgba(var(--v-theme-warning), 0.5);
}

.requirement.behind .v-icon {
  color: rgb(var(--v-theme-warning));
}

.restricted .dependent-header {
  align-items: center;
  color: rgb(var(--v-theme-on-surface-variant));
}

.restricted .dependent-name {
  color: rgb(var(--v-theme-on-surface-variant));
}

.restricted-hint {
  font-size: 14px;
  color: rgb(var(--v-theme-on-surface-variant));
}
</style>
