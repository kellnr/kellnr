<template>
  <header class="page-header" data-testid="page-header">
    <div class="page-header-main">
      <v-icon v-if="icon" :icon="icon" size="28" class="page-header-icon" aria-hidden="true"></v-icon>
      <div class="page-header-text">
        <h1 class="page-header-title" data-testid="page-title">{{ title }}</h1>
        <p v-if="subtitle" class="page-header-subtitle">{{ subtitle }}</p>
      </div>
    </div>
    <div v-if="$slots.actions" class="page-header-actions">
      <slot name="actions"></slot>
    </div>
  </header>
</template>

<script setup lang="ts">
defineProps<{
  title: string;
  icon?: string;
  subtitle?: string;
}>();
</script>

<style scoped>
.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
  margin-bottom: 24px;
}

.page-header-main {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.page-header-icon {
  color: rgb(var(--v-theme-primary));
}

.page-header-title {
  font-size: 1.5rem;
  font-weight: 700;
  line-height: 1.3;
  margin: 0;
}

.page-header-subtitle {
  margin: 4px 0 0;
  color: rgb(var(--v-theme-on-surface-variant));
  font-size: 0.875rem;
}

.page-header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
