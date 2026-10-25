<template>
  <v-card
    class="hero-stat-card"
    :class="{ 'clickable': props.onClick, 'compact': props.compact }"
    elevation="0"
    :ripple="!!props.onClick"
    :data-testid="`${props.testIdPrefix}-${textToTestId(text)}`"
    @click="handleClick">
    <v-card-text class="d-flex align-center ga-4 pa-4">
      <div class="icon-tile">
        <v-icon :icon="icon" :size="props.compact ? 22 : 26"></v-icon>
      </div>
      <div class="stat-content">
        <div class="font-weight-bold tabular-nums stat-number" :class="props.compact ? 'text-headline-small' : 'text-headline-medium'"
          data-testid="stat-value">
          {{ formattedNum }}
        </div>
        <div class="text-body-medium font-weight-medium stat-label" data-testid="stat-label">
          {{ text }}
        </div>
        <div v-if="subtitle" class="text-body-small stat-subtitle">
          {{ subtitle }}
        </div>
      </div>
      <v-icon v-if="props.onClick" icon="mdi-chevron-right" size="small" class="ms-auto chevron"></v-icon>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { formatNumber } from '../common/format';

const props = withDefaults(defineProps<{
  num: number | string;
  icon: string;
  text: string;
  subtitle?: string;
  /** Smaller variant for secondary statistics */
  compact?: boolean;
  testIdPrefix?: string;
  onClick?: () => void;
}>(), {
  compact: false,
  testIdPrefix: 'hero-stat',
});

function handleClick() {
  if (props.onClick) {
    props.onClick();
  }
}

const formattedNum = computed(() => formatNumber(props.num));

function textToTestId(text: string): string {
  return text.toLowerCase().replace(/\s+/g, '-');
}
</script>

<style scoped>
.hero-stat-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  transition: transform 0.2s ease, border-color 0.2s ease;
  cursor: default;
}

.hero-stat-card.clickable {
  cursor: pointer !important;
}

/* Only cards that lead somewhere react to the pointer */
.hero-stat-card.clickable:hover {
  transform: translateY(-2px);
  border-color: rgb(var(--v-theme-primary));
}

.icon-tile {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 52px;
  height: 52px;
  border-radius: var(--kellnr-radius);
  background: rgba(var(--v-theme-primary), 0.1);
  color: rgb(var(--v-theme-primary));
}

.compact .icon-tile {
  width: 44px;
  height: 44px;
}


.stat-content {
  min-width: 0;
}

.stat-number {
  line-height: 1.2;
  color: rgb(var(--v-theme-on-surface));
}

.stat-label {
  color: rgb(var(--v-theme-on-surface));
}

.stat-subtitle {
  color: rgb(var(--v-theme-on-surface-variant));
}

.chevron {
  color: rgb(var(--v-theme-on-surface-variant));
}

@media (prefers-reduced-motion: reduce) {
  .hero-stat-card {
    transition: none;
  }
}
</style>
