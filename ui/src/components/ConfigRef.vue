<template>
  <div class="config-ref" :class="{ 'source-active': active }">
    <span :class="['ref-badge', type]">{{ type.toUpperCase() }}</span>
    <code class="ref-value">{{ value }}</code>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  type: 'toml' | 'env' | 'cli';
  value: string;
  active: boolean;
}>();
</script>

<style scoped>
.config-ref {
  display: flex;
  align-items: center;
  gap: 8px;
  opacity: 0.5;
  transition: opacity 0.2s ease;
}

.config-ref.source-active {
  opacity: 1;
  font-weight: 600;
}

.config-ref.source-active .ref-badge {
  box-shadow: 0 0 0 2px currentColor;
}

.ref-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 42px;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.ref-badge.toml {
  background: rgba(103, 58, 183, 0.15);
  color: #7c4dff;
}

.ref-badge.env {
  background: rgba(0, 150, 136, 0.15);
  color: #00bfa5;
}

.ref-badge.cli {
  background: rgba(255, 152, 0, 0.15);
  color: #ff9800;
}

.ref-value {
  font-family: 'Roboto Mono', monospace;
  font-size: 12px;
  background: rgba(var(--v-theme-on-surface), 0.06);
  padding: 2px 6px;
  border-radius: 4px;
  color: rgb(var(--v-theme-on-surface-variant));
}
</style>
