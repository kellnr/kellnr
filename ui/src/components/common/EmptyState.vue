<template>
  <div class="empty-state" :class="{ 'empty-state--compact': compact }">
    <v-icon
      :icon="icon"
      :size="compact ? 'small' : 'large'"
      :class="compact ? 'me-2' : 'mb-3'"
      class="text-medium-emphasis"
    ></v-icon>
    <p class="text-body-2 text-medium-emphasis mb-0">{{ message }}</p>
  </div>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  icon: string;
  message: string;
  compact?: boolean;
}>(), {
  compact: false,
});
</script>

<style scoped>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 32px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px dashed rgb(var(--v-theme-outline));
  border-radius: 8px;
}

.empty-state--compact {
  flex-direction: row;
  justify-content: center;
  padding: 20px;
  background: rgb(var(--v-theme-surface));
  border-radius: 6px;
}
</style>
