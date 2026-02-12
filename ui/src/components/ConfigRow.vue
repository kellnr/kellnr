<template>
  <div class="config-row" v-show="visible">
    <div class="config-label" :class="{ 'configured': configured }">{{ label }}</div>
    <div class="config-value-cell">
      <div class="value-wrapper">
        <!-- Boolean values -->
        <span v-if="type === 'boolean'" :class="['boolean-badge', booleanClass]">
          {{ value ? 'true' : 'false' }}
        </span>
        <!-- Secret values -->
        <span v-else-if="secret" class="value-text secret">
          {{ value ? '********' : '-' }}
        </span>
        <!-- Regular values -->
        <span v-else class="value-text">{{ formattedValue }}</span>
        <!-- Default value shown below current value when configured -->
        <div v-if="configured" class="config-default">
          <span class="default-label">Default:</span>
          <span class="default-value">{{ formattedDefault }}</span>
        </div>
      </div>
    </div>
    <div class="config-refs">
      <ConfigRef type="toml" :value="configKey" :active="source === 'toml'" />
      <ConfigRef type="env" :value="envVar" :active="source === 'env'" />
      <ConfigRef v-if="cli" type="cli" :value="cli" :active="source === 'cli'" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import ConfigRef from './ConfigRef.vue';
import type { ConfigSource } from '../types/settings';

const props = defineProps<{
  configKey: string;
  label: string;
  value: unknown;
  defaultValue: unknown;
  source: ConfigSource;
  type?: 'boolean' | 'string' | 'number' | 'array';
  cli?: string;
  secret?: boolean;
  warningWhenTrue?: boolean;
  showOnlyConfigured: boolean;
}>();

const configured = computed(() => props.source !== 'default');
const visible = computed(() => !props.showOnlyConfigured || configured.value);

const envVar = computed(() => 'KELLNR_' + props.configKey.replace('.', '__').toUpperCase());

const formattedValue = computed(() => {
  const v = props.value;
  if (v === null || v === undefined) return '-';
  if (typeof v === 'boolean') return v ? 'true' : 'false';
  if (Array.isArray(v)) return v.length > 0 ? v.join(', ') : '-';
  return String(v) || '-';
});

const formattedDefault = computed(() => {
  const v = props.defaultValue;
  if (v === null || v === undefined || v === '') return 'unset';
  if (typeof v === 'boolean') return v ? 'true' : 'false';
  if (Array.isArray(v)) return v.length > 0 ? v.join(', ') : 'empty';
  return String(v);
});

const booleanClass = computed(() => {
  if (!configured.value) return 'default';
  if (props.warningWhenTrue && props.value) return 'warning';
  return props.value ? 'enabled' : 'disabled';
});
</script>

<style scoped>
.config-row {
  display: grid;
  grid-template-columns: 200px 160px 1fr;
  gap: 16px;
  align-items: start;
  padding: 12px 0;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.config-row:last-child {
  border-bottom: none;
}

.config-label {
  font-weight: 500;
  font-size: 14px;
  color: rgb(var(--v-theme-on-surface));
}

.config-label.configured {
  color: rgb(var(--v-theme-warning));
  font-weight: 600;
}

.config-value-cell {
  display: flex;
  align-items: flex-start;
}

.value-wrapper {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.value-text {
  font-family: 'Roboto Mono', monospace;
  font-size: 13px;
  background: rgba(var(--v-theme-primary), 0.08);
  color: rgb(var(--v-theme-on-surface));
  padding: 4px 8px;
  border-radius: 4px;
  word-break: break-all;
}

.value-text.secret {
  letter-spacing: 2px;
}

.boolean-badge {
  display: inline-flex;
  align-items: center;
  padding: 4px 10px;
  border-radius: 12px;
  font-size: 12px;
  font-weight: 600;
}

.boolean-badge.enabled {
  background: rgba(var(--v-theme-success), 0.15);
  color: rgb(var(--v-theme-success));
}

.boolean-badge.disabled,
.boolean-badge.default {
  background: rgba(var(--v-theme-on-surface), 0.08);
  color: rgb(var(--v-theme-on-surface-variant));
}

.boolean-badge.default {
  font-style: italic;
}

.boolean-badge.warning {
  background: rgba(var(--v-theme-warning), 0.15);
  color: rgb(var(--v-theme-warning));
}

.config-refs {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.config-default {
  display: flex;
  align-items: baseline;
  font-size: 12px;
  color: rgb(var(--v-theme-on-surface-variant));
}

.default-label {
  font-weight: 500;
  margin-right: 4px;
}

.default-value {
  font-family: 'Roboto Mono', monospace;
  font-style: italic;
}

@media (max-width: 960px) {
  .config-row {
    grid-template-columns: 1fr;
    gap: 8px;
  }

  .config-label {
    font-weight: 600;
  }

  .config-refs {
    flex-direction: row;
    flex-wrap: wrap;
  }
}
</style>
