<template>
  <div class="subsection-header">
    <v-icon :icon="icon" size="x-small" class="me-2" color="primary"></v-icon>
    <span class="text-body-2 font-weight-medium">{{ title }}</span>
    <span v-if="count !== undefined" class="subsection-count">{{ count }}</span>
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
.subsection-header {
  display: flex;
  align-items: center;
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.subsection-count {
  margin-left: auto;
  background: rgba(var(--v-theme-primary), 0.1);
  color: rgb(var(--v-theme-primary));
  font-size: 11px;
  font-weight: 500;
  padding: 2px 8px;
  border-radius: 10px;
}
</style>
