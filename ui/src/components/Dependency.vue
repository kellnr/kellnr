<template>
  <div class="dependency-item" @click="openCratePage">
    <div class="dep-main">
      <!-- Left: Name, Version, Source -->
      <div class="dep-info">
        <div class="dep-header">
          <span class="dep-name">{{ name }}</span>
          <span class="dep-version">{{ version }}</span>
        </div>
        <div class="dep-source">
          <span class="source-badge" :class="isCratesIoDep(registry) ? 'crates-io' : 'kellnr'">
            <v-icon :icon="isCratesIoDep(registry) ? 'mdi-package-variant' : 'mdi-package-variant-closed'" size="x-small" class="me-1"></v-icon>
            {{ isCratesIoDep(registry) ? 'crates.io' : 'kellnr' }}
          </span>
        </div>
      </div>

      <!-- Right: Description -->
      <div class="dep-description">
        <span v-if="fetched_desc">{{ fetched_desc }}</span>
        <span v-else class="loading-state">
          <v-progress-circular indeterminate size="14" width="2" color="primary" class="me-2"></v-progress-circular>
          Loading...
        </span>
      </div>
    </div>

    <!-- Chevron indicator -->
    <v-icon icon="mdi-chevron-right" size="small" class="dep-chevron"></v-icon>
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { crateService } from "../services";
import { isSuccess } from "../services/api";
import { CRATESIO_LINK } from "../remote-routes";

const props = defineProps<{
  name: string
  version: string
  registry?: string
  desc?: string
}>()

const fetched_desc = ref("")
const route = useRoute()
const router = useRouter()

onBeforeMount(() => {
  if (!props.desc) {
    setDesc(props.name, props.registry);
  } else {
    fetched_desc.value = props.desc
  }
})

function getCratesIoUrl(crate: string) {
  return CRATESIO_LINK(crate);
}

function isCratesIoDep(registry: string | undefined) {
  return registry === "https://github.com/rust-lang/crates.io-index";
}

async function setDescFromCratesIo(crate: string) {
  const result = await crateService.getCratesIoData(crate);
  if (isSuccess(result) && result.data) {
    // The crates.io response wraps crate data in a 'crate' property
    const data = result.data as unknown as { crate: { description: string } };
    fetched_desc.value = data.crate?.description ?? "No description available";
  } else {
    fetched_desc.value = "Cannot fetch description.";
  }
}

async function setDescFromKellnr(crate: string) {
  const result = await crateService.getCrateData(crate);
  if (isSuccess(result) && result.data) {
    fetched_desc.value = result.data.description ?? "No description set";
  } else {
    fetched_desc.value = "Cannot fetch description.";
  }
}

function setDesc(crate: string, registry: string | undefined) {
  if (isCratesIoDep(registry)) {
    setDescFromCratesIo(crate);
  } else {
    setDescFromKellnr(crate);
  }
}

function openCratesIoPage() {
  window.open(getCratesIoUrl(props.name));
}

function openKellnrPage() {
  router.push({ name: 'Crate', query: { name: props.name } })
}

function openCratePage() {
  if (isCratesIoDep(props.registry)) {
    openCratesIoPage();
  } else {
    openKellnrPage();
  }
}

// Watches route changes and reloads the data.
// Needed, if the query parameter "name=crate" changes.
watch(route, () => {
  setDesc(props.name, props.registry)
})
</script>

<style scoped>
.dependency-item {
  display: flex;
  align-items: center;
  padding: 12px 16px;
  margin-bottom: 8px;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.dependency-item:hover {
  background: rgb(var(--v-theme-surface));
  border-color: rgb(var(--v-theme-primary));
}

.dependency-item:last-child {
  margin-bottom: 0;
}

.dep-main {
  flex: 1;
  display: flex;
  align-items: flex-start;
  gap: 16px;
  min-width: 0;
}

.dep-info {
  flex-shrink: 0;
  min-width: 180px;
  max-width: 220px;
}

.dep-header {
  display: flex;
  align-items: baseline;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 6px;
}

.dep-name {
  font-weight: 600;
  font-size: 16px;
  color: rgb(var(--v-theme-on-surface));
}

.dep-version {
  font-size: 15px;
  font-weight: 500;
  color: rgb(var(--v-theme-primary));
  font-family: 'Roboto Mono', monospace;
}

.dep-source {
  display: flex;
}

.source-badge {
  display: inline-flex;
  align-items: center;
  padding: 3px 10px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}

.source-badge.crates-io {
  background: rgba(255, 152, 0, 0.15);
  color: rgb(var(--v-theme-warning));
}

.source-badge.kellnr {
  background: rgba(var(--v-theme-primary), 0.15);
  color: rgb(var(--v-theme-primary));
}

.dep-description {
  flex: 1;
  font-size: 15px;
  line-height: 1.5;
  color: rgb(var(--v-theme-on-surface-variant));
  min-width: 0;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.loading-state {
  display: inline-flex;
  align-items: center;
  font-style: italic;
  color: rgb(var(--v-theme-on-surface-variant));
}

.dep-chevron {
  flex-shrink: 0;
  color: rgb(var(--v-theme-on-surface-variant));
  margin-left: 12px;
  opacity: 0.5;
  transition: all 0.2s ease;
}

.dependency-item:hover .dep-chevron {
  opacity: 1;
  color: rgb(var(--v-theme-primary));
  transform: translateX(2px);
}

/* Responsive adjustments */
@media (max-width: 768px) {
  .dep-main {
    flex-direction: column;
    gap: 8px;
  }

  .dep-info {
    min-width: auto;
    max-width: none;
  }

  .dep-description {
    -webkit-line-clamp: 3;
  }
}

@media (max-width: 480px) {
  .dependency-item {
    padding: 10px 12px;
  }

  .dep-header {
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
  }
}
</style>
