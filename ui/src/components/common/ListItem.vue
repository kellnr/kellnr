<template>
  <div class="list-item" :class="{ 'list-item--compact': compact }" :data-testid="testId">
    <div class="list-item-content">
      <div v-if="$slots.avatar || icon" class="list-item-avatar" :class="avatarClass">
        <slot name="avatar">
          <v-icon :icon="icon" size="small"></v-icon>
        </slot>
      </div>
      <div class="list-item-details">
        <slot name="title">
          <span class="list-item-title">{{ title }}</span>
        </slot>
        <slot name="subtitle"></slot>
      </div>
      <slot name="badges"></slot>
    </div>
    <div v-if="$slots.actions" class="list-item-actions">
      <slot name="actions"></slot>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = withDefaults(defineProps<{
  icon?: string;
  title?: string;
  avatarColor?: 'primary' | 'success' | 'warning' | 'error' | 'info';
  compact?: boolean;
  testId?: string;
}>(), {
  avatarColor: 'primary',
  compact: false,
});

const avatarClass = computed(() => `avatar-${props.avatarColor}`);
</script>

<style scoped>
.list-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  transition: all 0.2s ease;
}

.list-item:hover {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.06);
}

.list-item--compact {
  padding: 10px 14px;
  background: rgb(var(--v-theme-surface));
  border-radius: 6px;
}

.list-item--compact:hover {
  background: rgba(var(--v-theme-primary), 0.03);
}

.list-item-content {
  display: flex;
  align-items: center;
  gap: 12px;
}

.list-item-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
}

.list-item--compact .list-item-avatar {
  width: 28px;
  height: 28px;
  border-radius: 6px;
}

.avatar-primary {
  background: rgba(var(--v-theme-primary), 0.15);
  color: rgb(var(--v-theme-primary));
}

.avatar-success {
  background: rgba(var(--v-theme-success), 0.15);
  color: rgb(var(--v-theme-success));
}

.avatar-warning {
  background: rgba(var(--v-theme-warning), 0.15);
  color: rgb(var(--v-theme-warning));
}

.avatar-error {
  background: rgba(var(--v-theme-error), 0.15);
  color: rgb(var(--v-theme-error));
}

.avatar-info {
  background: rgba(var(--v-theme-info), 0.15);
  color: rgb(var(--v-theme-info));
}

.list-item-details {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.list-item-title {
  font-weight: 600;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.list-item--compact .list-item-title {
  font-weight: 500;
  font-size: 14px;
}

.list-item-actions {
  display: flex;
  gap: 8px;
}

@media (max-width: 768px) {
  .list-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }

  .list-item-actions {
    width: 100%;
    justify-content: flex-start;
  }
}
</style>
