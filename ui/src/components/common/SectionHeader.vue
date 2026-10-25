<template>
  <div class="section-header">
    <v-icon :icon="icon" size="small" class="me-3 section-icon"></v-icon>
    <span class="section-title" data-testid="section-title">{{ title }}</span>
    <span v-if="count !== undefined" class="section-count">{{ count }}</span>
    <slot name="actions"></slot>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  icon: string;
  title: string;
  count?: number;
}>();
</script>

<style scoped>
.section-header {
  display: flex;
  align-items: center;
  padding: 16px 24px;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.section-title {
  font-size: 1.0625rem;
  font-weight: 600;
}

.section-icon {
  color: rgb(var(--v-theme-on-surface-variant));
}

.section-count {
  margin-left: auto;
  background: rgb(var(--v-theme-surface-variant));
  color: rgb(var(--v-theme-on-surface-variant));
  font-size: 12px;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 9999px;
}

@media (max-width: 600px) {
  .section-header {
    padding: 16px 20px;
  }
}
</style>
