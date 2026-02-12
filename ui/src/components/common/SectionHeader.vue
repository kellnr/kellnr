<template>
  <div class="section-header">
    <v-icon :icon="icon" size="small" color="primary" class="me-3"></v-icon>
    <span class="text-h6 font-weight-bold">{{ title }}</span>
    <span v-if="count !== undefined && count > 0" class="section-count">{{ count }}</span>
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
  background: rgba(var(--v-theme-primary), 0.05);
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.section-count {
  margin-left: auto;
  background: rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-primary));
  font-size: 12px;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 12px;
}

@media (max-width: 600px) {
  .section-header {
    padding: 16px 20px;
  }
}
</style>
