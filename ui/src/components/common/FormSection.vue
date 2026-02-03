<template>
  <div class="form-section">
    <SubsectionHeader :icon="icon" :title="title" />
    <slot></slot>
  </div>
</template>

<script setup lang="ts">
import SubsectionHeader from './SubsectionHeader.vue';

defineProps<{
  icon: string;
  title: string;
}>();
</script>

<style scoped>
.form-section {
  padding: 20px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
}
</style>
