<template>
  <v-card class="crate-list-card h-100" elevation="0" :data-testid="testId">
    <SectionHeader :icon="icon" :title="title" />
    <v-list class="py-1" density="compact" bg-color="transparent">
      <v-list-item v-for="crate in crates" :key="crate.name" class="crate-row px-6" :data-testid="rowTestId"
        @click="openCrate(crate.name)">
        <div class="d-flex align-center ga-3">
          <div class="crate-text">
            <span class="crate-name">{{ crate.name }}</span>
            <span class="crate-version">{{ crate.version }}</span>
          </div>
          <span class="crate-meta" :title="metaTitle(crate)">{{ metaText(crate) }}</span>
        </div>
      </v-list-item>
    </v-list>
    <div class="px-6 pb-3">
      <router-link :to="viewAllRoute" class="view-all" :data-testid="`${testId}-view-all`">
        View all
        <v-icon icon="mdi-arrow-right" size="x-small" />
      </router-link>
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import type { CrateOverview } from "../types/crate_overview";
import type { CrateSort } from "../types/api";
import { SectionHeader } from "./common";
import { formatNumber } from "../common/format";
import { useRelativeTime } from "../composables";

const props = defineProps<{
  title: string
  icon: string
  crates: CrateOverview[]
  /** Shown next to each crate, and the order of the full list behind "View all" */
  sort: Extract<CrateSort, "updated" | "downloads">
  testId: string
  rowTestId: string
}>();

const router = useRouter();
const { fromNow, exactDate } = useRelativeTime();

const viewAllRoute = computed(() => ({ path: "/crates", query: { sort: props.sort } }));

function metaText(crate: CrateOverview): string {
  if (props.sort === "updated") {
    return fromNow(crate.date);
  }
  const noun = crate.total_downloads === 1 ? "download" : "downloads";
  return `${formatNumber(crate.total_downloads)} ${noun}`;
}

function metaTitle(crate: CrateOverview): string | undefined {
  return props.sort === "updated" ? exactDate(crate.date) : undefined;
}

function openCrate(name: string) {
  router.push({ name: "Crate", query: { name } });
}
</script>

<style scoped>
.crate-list-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
}

.crate-row {
  min-height: 44px;
}

.crate-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.crate-name {
  font-weight: 600;
  color: rgb(var(--v-theme-on-surface));
}

.crate-version {
  margin-left: 8px;
  font-size: 0.8125rem;
  color: rgb(var(--v-theme-on-surface-variant));
}

.crate-meta {
  flex-shrink: 0;
  font-size: 0.8125rem;
  color: rgb(var(--v-theme-on-surface-variant));
}

.view-all {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 0.875rem;
  font-weight: 500;
  color: rgb(var(--v-theme-primary));
  text-decoration: none;
}

.view-all:hover {
  text-decoration: underline;
}
</style>
