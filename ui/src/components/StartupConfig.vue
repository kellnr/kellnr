<template>
  <div>
    <!-- Header -->
    <div class="section-header">
      <v-icon icon="mdi-tune" size="small" color="primary" class="me-3"></v-icon>
      <span class="text-h6 font-weight-bold">Startup Configuration</span>
      <span v-if="totalConfigured > 0" class="section-count configured">{{ totalConfigured }}/{{ totalSettings }} configured</span>
      <span class="section-count">{{ visibleSectionsCount }} sections</span>
    </div>

    <!-- Content -->
    <div class="section-content">
      <p class="text-body-2 text-medium-emphasis mb-4">
        Configuration values are set on application startup and cannot be changed at runtime.
        See the <a href="https://kellnr.io/documentation" class="text-primary font-weight-medium">Kellnr Documentation</a> for details.
      </p>

      <ConfigToolbar
        v-model:filter-active="showOnlyConfigured"
        @expand-all="expandAll"
        @collapse-all="collapseAll"
      />

      <!-- Config Sections -->
      <v-expansion-panels v-model="expandedPanels" multiple class="config-panels">
        <v-expansion-panel
          v-for="section in sections"
          :key="section.key"
          v-show="isSectionVisible(section.key)"
        >
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon :icon="section.icon" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">{{ section.title }}</span>
              <v-chip
                v-if="sectionEnabledLeaf(section.key)"
                size="x-small"
                class="ms-2"
                :color="sectionEnabledLeaf(section.key)?.value ? 'success' : 'default'"
                variant="tonal"
              >
                {{ sectionEnabledLeaf(section.key)?.value ? 'Enabled' : 'Disabled' }}
              </v-chip>
              <v-chip
                size="x-small"
                class="ms-2"
                :color="configuredCounts[section.key] > 0 ? 'success' : 'primary'"
                variant="tonal"
              >
                {{ badgeText(section.key) }}
              </v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <ConfigRow
                v-for="item in section.items"
                :key="item.key"
                :config-key="item.key"
                :label="leafLabel(item)"
                :value="item.value"
                :default-value="item.default"
                :source="item.source"
                :type="item.type"
                :cli="item.cli_flag ?? undefined"
                :secret="item.secret"
                :warning-when-true="isWarningWhenTrue(item.key)"
                :show-only-configured="showOnlyConfigured"
              />
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref, computed } from "vue";
import { emptySettings } from "../types/settings";
import type { Settings, LeafMeta } from "../types/settings";
import { settingsService } from "../services";
import { isSuccess } from "../services/api";
import ConfigRow from "./ConfigRow.vue";
import ConfigToolbar from "./ConfigToolbar.vue";

// Presentational metadata only, keys, types, CLI flags, secrets, and labels
// now come from the backend's `leaves` field. The order of this array defines
// the section display order; an `enabled` leaf in a section is treated as the
// section's toggle automatically.
interface SectionDescriptor {
  key: string;
  title: string;
  icon: string;
}

const sectionDescriptors: SectionDescriptor[] = [
  { key: 'registry', title: 'Registry', icon: 'mdi-package-variant-closed' },
  { key: 'local', title: 'Local', icon: 'mdi-server' },
  { key: 'origin', title: 'Origin', icon: 'mdi-earth' },
  { key: 'log', title: 'Log', icon: 'mdi-file-document-outline' },
  { key: 'proxy', title: 'Proxy', icon: 'mdi-transit-connection-variant' },
  { key: 'docs', title: 'Docs', icon: 'mdi-file-document-multiple-outline' },
  { key: 'postgresql', title: 'PostgreSQL', icon: 'mdi-database' },
  { key: 's3', title: 'S3 Storage', icon: 'mdi-cloud-outline' },
  { key: 'toolchain', title: 'Toolchain', icon: 'mdi-wrench' },
];

// Leaves whose boolean `true` value should render with a warning style
// (currently just `s3.allow_http`). Kept here because it's a pure UI hint.
const WARNING_WHEN_TRUE = new Set<string>(['s3.allow_http']);

// Hidden leaves: sections kellnr defines but the startup-config screen
// deliberately doesn't expose (e.g. setup credentials, oauth2, those have
// their own admin screens).
const HIDDEN_SECTIONS = new Set<string>(['setup', 'oauth2']);

const settings = ref<Settings>(emptySettings);
const showOnlyConfigured = ref(false);
const expandedPanels = ref<number[]>([]);

// Group leaves by section (the part before the first dot) so the template can
// iterate `sections[i].items` without re-filtering on every render.
const sections = computed(() => {
  const grouped = new Map<string, LeafMeta[]>();
  for (const leaf of settings.value.leaves ?? []) {
    const section = leaf.key.split('.')[0];
    if (HIDDEN_SECTIONS.has(section)) continue;
    const list = grouped.get(section);
    if (list) list.push(leaf);
    else grouped.set(section, [leaf]);
  }
  // Project the section descriptors so the order matches `sectionDescriptors`.
  return sectionDescriptors
    .map(d => ({ ...d, items: grouped.get(d.key) ?? [] }))
    .filter(s => s.items.length > 0);
});

const configuredCounts = computed(() => {
  const counts: Record<string, number> = {};
  for (const section of sections.value) {
    counts[section.key] = section.items.filter(item => item.source !== 'default').length;
  }
  return counts;
});

const totalConfigured = computed(() => Object.values(configuredCounts.value).reduce((sum, n) => sum + n, 0));
const totalSettings = computed(() => sections.value.reduce((sum, s) => sum + s.items.length, 0));
const visibleSectionsCount = computed(() => {
  if (!showOnlyConfigured.value) return sections.value.length;
  return sections.value.filter(s => configuredCounts.value[s.key] > 0).length;
});

// `data_dir` → `Data Directory`. Used only when the backend doesn't supply
// a `label` override.
function humanize(field: string): string {
  return field
    .split('_')
    .filter(word => word.length > 0)
    .map(word => word[0].toUpperCase() + word.slice(1))
    .join(' ');
}

function leafLabel(leaf: LeafMeta): string {
  if (leaf.label) return leaf.label;
  const field = leaf.key.split('.').slice(1).join('.');
  return humanize(field);
}

function sectionEnabledLeaf(sectionKey: string): LeafMeta | undefined {
  return sections.value
    .find(s => s.key === sectionKey)
    ?.items.find(l => l.key === `${sectionKey}.enabled`);
}

function isSectionVisible(sectionKey: string): boolean {
  if (!showOnlyConfigured.value) return true;
  return (configuredCounts.value[sectionKey] ?? 0) > 0;
}

function badgeText(sectionKey: string): string {
  const section = sections.value.find(s => s.key === sectionKey);
  if (!section) return '';
  const configured = configuredCounts.value[sectionKey];
  const total = section.items.length;
  return configured > 0 ? `${configured}/${total} configured` : `${total} settings`;
}

const visibleSectionIndices = computed(() =>
  sections.value
    .map((s, i) => ({ key: s.key, index: i }))
    .filter(({ key }) => isSectionVisible(key))
    .map(({ index }) => index)
);

function expandAll() {
  expandedPanels.value = [...visibleSectionIndices.value];
}

function collapseAll() {
  expandedPanels.value = [];
}

function isWarningWhenTrue(key: string): boolean {
  return WARNING_WHEN_TRUE.has(key);
}

onBeforeMount(async () => {
  const result = await settingsService.getSettings();
  if (isSuccess(result)) {
    settings.value = result.data;
  }
});
</script>

<style scoped>
.section-header {
  display: flex;
  align-items: center;
  padding: 16px 24px;
  background: rgba(var(--v-theme-primary), 0.05);
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.section-count {
  margin-left: 8px;
  background: rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-primary));
  font-size: 12px;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 12px;
}

.section-count:first-of-type {
  margin-left: auto;
}

.section-count.configured {
  background: rgb(var(--v-theme-success));
  color: rgb(var(--v-theme-on-success));
}

.section-content {
  padding: 24px;
}

.config-panels {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.config-panels :deep(.v-expansion-panel) {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px !important;
  margin-bottom: 0;
}

.config-panels :deep(.v-expansion-panel::before) {
  box-shadow: none;
}

.config-panels :deep(.v-expansion-panel--active) {
  border-color: rgb(var(--v-theme-primary));
}

.panel-title {
  min-height: 56px;
}

.panel-header {
  display: flex;
  align-items: center;
}

.panel-icon-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background: rgba(var(--v-theme-primary), 0.1);
  color: rgb(var(--v-theme-primary));
  margin-right: 12px;
}

.config-list {
  display: flex;
  flex-direction: column;
}

@media (max-width: 600px) {
  .section-header {
    padding: 16px 20px;
  }

  .section-content {
    padding: 20px;
  }
}
</style>
