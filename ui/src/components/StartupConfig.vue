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
                v-if="section.hasEnabledToggle"
                size="x-small"
                class="ms-2"
                :color="getSettingValue(section.key + '.enabled') ? 'success' : 'default'"
                variant="tonal"
              >
                {{ getSettingValue(section.key + '.enabled') ? 'Enabled' : 'Disabled' }}
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
                :label="item.label"
                :value="getSettingValue(item.key)"
                :default-value="getDefaultValue(item.key)"
                :source="getSource(item.key)"
                :type="item.type"
                :cli="item.cli"
                :secret="item.secret"
                :warning-when-true="item.warningWhenTrue"
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
import type { Settings, ConfigSource } from "../types/settings";
import { settingsService } from "../services";
import { isSuccess } from "../services/api";
import ConfigRow from "./ConfigRow.vue";
import ConfigToolbar from "./ConfigToolbar.vue";

// Types for config schema
interface ConfigItem {
  key: string;
  label: string;
  type?: 'boolean' | 'string' | 'number' | 'array';
  cli?: string;
  secret?: boolean;
  warningWhenTrue?: boolean;
}

interface ConfigSection {
  key: string;
  title: string;
  icon: string;
  hasEnabledToggle?: boolean;
  items: ConfigItem[];
}

// Configuration schema
const sections: ConfigSection[] = [
  {
    key: 'registry',
    title: 'Registry',
    icon: 'mdi-package-variant-closed',
    items: [
      { key: 'registry.data_dir', label: 'Data Directory', cli: '--registry-data-dir, -d' },
      { key: 'registry.session_age_seconds', label: 'Session Age (seconds)', cli: '--registry-session-age' },
      { key: 'registry.cache_size', label: 'Cache Size', cli: '--registry-cache-size' },
      { key: 'registry.max_crate_size', label: 'Max Crate Size', cli: '--registry-max-crate-size' },
      { key: 'registry.max_db_connections', label: 'Max DB Connections', cli: '--registry-max-db-connections' },
      { key: 'registry.auth_required', label: 'Auth Required', type: 'boolean', cli: '--registry-auth-required' },
      { key: 'registry.required_crate_fields', label: 'Required Crate Fields', type: 'array', cli: '--registry-required-crate-fields' },
      { key: 'registry.new_crates_restricted', label: 'New Crates Restricted', type: 'boolean', cli: '--registry-new-crates-restricted' },
      { key: 'registry.cookie_signing_key', label: 'Cookie Signing Key', secret: true, cli: '--registry-cookie-signing-key' },
      { key: 'registry.allow_ownerless_crates', label: 'Allow Ownerless Crates', type: 'boolean', cli: '--registry-allow-ownerless-crates' },
      { key: 'registry.token_cache_enabled', label: 'Token Cache Enabled', type: 'boolean', cli: '--registry-token-cache-enabled' },
      { key: 'registry.token_cache_ttl_seconds', label: 'Token Cache TTL (seconds)', cli: '--registry-token-cache-ttl' },
      { key: 'registry.token_cache_max_capacity', label: 'Token Cache Max Capacity', cli: '--registry-token-cache-max-capacity' },
      { key: 'registry.token_db_retry_count', label: 'Token DB Retry Count', cli: '--registry-token-db-retry-count' },
      { key: 'registry.token_db_retry_delay_ms', label: 'Token DB Retry Delay (ms)', cli: '--registry-token-db-retry-delay' },
    ]
  },
  {
    key: 'local',
    title: 'Local',
    icon: 'mdi-server',
    items: [
      { key: 'local.ip', label: 'IP', cli: '--local-ip' },
      { key: 'local.port', label: 'Port', cli: '--local-port, -p' },
    ]
  },
  {
    key: 'origin',
    title: 'Origin',
    icon: 'mdi-earth',
    items: [
      { key: 'origin.hostname', label: 'Hostname', cli: '--origin-hostname' },
      { key: 'origin.port', label: 'Port', cli: '--origin-port' },
      { key: 'origin.protocol', label: 'Protocol' },
      { key: 'origin.path', label: 'Path', cli: '--origin-path' },
    ]
  },
  {
    key: 'log',
    title: 'Log',
    icon: 'mdi-file-document-outline',
    items: [
      { key: 'log.level', label: 'Level', cli: '--log-level, -l' },
      { key: 'log.format', label: 'Format', cli: '--log-format' },
      { key: 'log.level_web_server', label: 'Level Web Server', cli: '--log-level-web-server' },
    ]
  },
  {
    key: 'proxy',
    title: 'Proxy',
    icon: 'mdi-transit-connection-variant',
    hasEnabledToggle: true,
    items: [
      { key: 'proxy.enabled', label: 'Enabled', type: 'boolean', cli: '--proxy-enabled' },
      { key: 'proxy.num_threads', label: 'Number of Threads', cli: '--proxy-num-threads' },
      { key: 'proxy.download_on_update', label: 'Download on Update', type: 'boolean', cli: '--proxy-download-on-update' },
      { key: 'proxy.url', label: 'URL', cli: '--proxy-url' },
      { key: 'proxy.index', label: 'Index URL', cli: '--proxy-index' },
    ]
  },
  {
    key: 'docs',
    title: 'Docs',
    icon: 'mdi-file-document-multiple-outline',
    hasEnabledToggle: true,
    items: [
      { key: 'docs.enabled', label: 'Enabled', type: 'boolean', cli: '--docs-enabled' },
      { key: 'docs.max_size', label: 'Max Size', cli: '--docs-max-size' },
    ]
  },
  {
    key: 'postgresql',
    title: 'PostgreSQL',
    icon: 'mdi-database',
    hasEnabledToggle: true,
    items: [
      { key: 'postgresql.enabled', label: 'Enabled', type: 'boolean', cli: '--postgresql-enabled' },
      { key: 'postgresql.address', label: 'Address', cli: '--postgresql-address' },
      { key: 'postgresql.port', label: 'Port', cli: '--postgresql-port' },
      { key: 'postgresql.db', label: 'Database', cli: '--postgresql-db' },
      { key: 'postgresql.user', label: 'User', cli: '--postgresql-user' },
    ]
  },
  {
    key: 's3',
    title: 'S3 Storage',
    icon: 'mdi-cloud-outline',
    hasEnabledToggle: true,
    items: [
      { key: 's3.enabled', label: 'Enabled', type: 'boolean', cli: '--s3-enabled' },
      { key: 's3.access_key', label: 'Access Key', cli: '--s3-access-key' },
      { key: 's3.secret_key', label: 'Secret Key', secret: true, cli: '--s3-secret-key' },
      { key: 's3.region', label: 'Region', cli: '--s3-region' },
      { key: 's3.endpoint', label: 'Endpoint', cli: '--s3-endpoint' },
      { key: 's3.allow_http', label: 'Allow HTTP', type: 'boolean', warningWhenTrue: true, cli: '--s3-allow-http' },
      { key: 's3.crates_bucket', label: 'Crates Bucket', cli: '--s3-crates-bucket' },
      { key: 's3.cratesio_bucket', label: 'Crates.io Bucket', cli: '--s3-cratesio-bucket' },
      { key: 's3.toolchain_bucket', label: 'Toolchain Bucket', cli: '--s3-toolchain-bucket' },
    ]
  },
  {
    key: 'toolchain',
    title: 'Toolchain',
    icon: 'mdi-wrench',
    hasEnabledToggle: true,
    items: [
      { key: 'toolchain.enabled', label: 'Enabled', type: 'boolean', cli: '--toolchain-enabled' },
      { key: 'toolchain.max_size', label: 'Max Size (MB)', cli: '--toolchain-max-size' },
    ]
  },
];

const settings = ref<Settings>(emptySettings);
const showOnlyConfigured = ref(false);
const expandedPanels = ref<number[]>([]);

// Computed properties
const configuredCounts = computed(() => {
  const counts: Record<string, number> = {};
  for (const section of sections) {
    counts[section.key] = section.items.filter(item => getSource(item.key) !== 'default').length;
  }
  return counts;
});

const totalConfigured = computed(() => Object.values(configuredCounts.value).reduce((sum, n) => sum + n, 0));
const totalSettings = computed(() => sections.reduce((sum, s) => sum + s.items.length, 0));
const visibleSectionsCount = computed(() => {
  if (!showOnlyConfigured.value) return sections.length;
  return sections.filter(s => configuredCounts.value[s.key] > 0).length;
});

// Helper functions
function getSettingValue(key: string): unknown {
  const [section, field] = key.split('.');
  const sectionObj = settings.value[section as keyof Settings];
  if (!sectionObj || typeof sectionObj !== 'object') return undefined;
  return (sectionObj as Record<string, unknown>)[field];
}

function getDefaultValue(key: string): unknown {
  if (!settings.value.defaults) return undefined;
  const [section, field] = key.split('.');
  const sectionObj = settings.value.defaults[section as keyof typeof settings.value.defaults];
  if (!sectionObj || typeof sectionObj !== 'object') return undefined;
  return (sectionObj as Record<string, unknown>)[field];
}

function getSource(key: string): ConfigSource {
  return settings.value.sources[key] || 'default';
}

function isSectionVisible(sectionKey: string): boolean {
  if (!showOnlyConfigured.value) return true;
  return configuredCounts.value[sectionKey] > 0;
}

function badgeText(sectionKey: string): string {
  const section = sections.find(s => s.key === sectionKey);
  if (!section) return '';
  const configured = configuredCounts.value[sectionKey];
  const total = section.items.length;
  return configured > 0 ? `${configured}/${total} configured` : `${total} settings`;
}

// Expand/collapse
const visibleSectionIndices = computed(() =>
  sections
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
