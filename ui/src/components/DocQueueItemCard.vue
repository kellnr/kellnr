<template>
  <div class="queue-item" :class="{ 'last-item': isLast }">
    <div class="queue-item-content">
      <div class="index-badge">
        <span>{{ index }}</span>
      </div>
      <div class="crate-info">
        <router-link class="crate-link" :to="{ name: 'Crate', query: { name: name, version: version } }">
          {{ name }}
        </router-link>
      </div>
      <div class="version-info">
        <v-chip size="small" variant="tonal" color="primary" class="version-chip">
          v{{ version }}
        </v-chip>
      </div>
      <div class="status-info">
        <v-icon icon="mdi-timer-sand" size="small" class="status-icon" />
        <span class="status-text">Pending</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  index: number
  name: string
  version: string
  isLast?: boolean
}>()
</script>

<style scoped>
.queue-item {
  padding: 16px 20px;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
  transition: background-color 0.2s ease;
}

.queue-item:hover {
  background: rgba(var(--v-theme-primary), 0.03);
}

.queue-item.last-item {
  border-bottom: none;
}

.queue-item-content {
  display: grid;
  grid-template-columns: 40px 1fr auto auto;
  align-items: center;
  gap: 16px;
}

.index-badge {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(var(--v-theme-primary), 0.1);
  color: rgb(var(--v-theme-primary));
  border-radius: 8px;
  font-weight: 600;
  font-size: 0.875rem;
}

.crate-info {
  min-width: 0;
}

.crate-link {
  font-size: 1rem;
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface));
  text-decoration: none;
  transition: color 0.2s ease;
}

.crate-link:hover {
  color: rgb(var(--v-theme-primary));
  text-decoration: underline;
}

.version-info {
  flex-shrink: 0;
}

.version-chip {
  font-size: 0.75rem;
  font-weight: 500;
}

.status-info {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.status-icon {
  color: rgb(var(--v-theme-warning));
}

.status-text {
  font-size: 0.8rem;
  color: rgb(var(--v-theme-on-surface-variant));
  font-weight: 500;
}

/* Responsive adjustments */
@media (max-width: 600px) {
  .queue-item-content {
    grid-template-columns: 32px 1fr;
    gap: 12px;
  }

  .version-info,
  .status-info {
    grid-column: 2;
  }

  .version-info {
    margin-top: -8px;
  }

  .status-info {
    margin-top: 4px;
  }
}
</style>
