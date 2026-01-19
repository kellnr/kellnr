<template>
  <v-card
    class="hero-stat-card"
    :class="{ 'clickable': props.onClick }"
    :style="cardStyle"
    elevation="4"
    rounded="xl"
    :ripple="!!props.onClick"
    :data-testid="`hero-stat-${textToTestId(text)}`"
    @click="handleClick">
    <v-card-text class="d-flex flex-column h-100 pa-5" :class="textColorClass">
      <div class="d-flex justify-space-between align-start mb-auto">
        <div class="stat-content">
          <div class="text-h2 font-weight-bold stat-number" data-testid="stat-value">
            {{ formattedNum }}
          </div>
          <div class="text-h6 font-weight-medium mt-1 stat-label" data-testid="stat-label">
            {{ text }}
          </div>
        </div>
        <v-avatar :color="getIconBgColor" size="72" class="elevation-2 icon-avatar">
          <v-icon :color="getIconColor" :icon="icon" size="x-large"></v-icon>
        </v-avatar>
      </div>
      <div v-if="subtitle" class="text-body-2 mt-3 stat-subtitle">
        {{ subtitle }}
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useTheme } from 'vuetify';

const props = defineProps<{
  num: number | string;
  icon: string;
  text: string;
  subtitle?: string;
  category?: 'primary' | 'secondary' | 'accent';
  onClick?: () => void;
}>();

function handleClick() {
  if (props.onClick) {
    props.onClick();
  }
}

const theme = useTheme();
const isDark = computed(() => theme.global.current.value.dark);

// Format large numbers with commas
const formattedNum = computed(() => {
  if (typeof props.num === 'number') {
    return props.num.toLocaleString();
  }
  return props.num;
});

const textColorClass = computed(() => {
  return isDark.value ? 'text-dark-theme' : 'text-light-theme';
});

const cardStyle = computed(() => {
  if (isDark.value) {
    // Dark mode: deeper, richer colors
    switch (props.category) {
      case 'primary':
        return { background: 'linear-gradient(145deg, #1565c0 0%, #0d47a1 50%, #0a3d91 100%)' };
      case 'secondary':
        return { background: 'linear-gradient(145deg, #7b1fa2 0%, #6a1b9a 50%, #4a148c 100%)' };
      case 'accent':
        return { background: 'linear-gradient(145deg, #00897b 0%, #00796b 50%, #004d40 100%)' };
      default:
        return { background: 'linear-gradient(145deg, #1565c0 0%, #0d47a1 50%, #0a3d91 100%)' };
    }
  }

  // Light mode: softer, more muted colors that are easier on the eyes
  switch (props.category) {
    case 'primary':
      return { background: 'linear-gradient(145deg, #5c8ec4 0%, #4a7ab0 50%, #3d6a9c 100%)' };
    case 'secondary':
      return { background: 'linear-gradient(145deg, #9575a8 0%, #7d5f8f 50%, #6b5080 100%)' };
    case 'accent':
      return { background: 'linear-gradient(145deg, #5a9e95 0%, #4a8a81 50%, #3d7a71 100%)' };
    default:
      return { background: 'linear-gradient(145deg, #5c8ec4 0%, #4a7ab0 50%, #3d6a9c 100%)' };
  }
});

const getIconBgColor = computed(() => {
  return isDark.value ? 'rgba(255, 255, 255, 0.15)' : 'rgba(255, 255, 255, 0.25)';
});

const getIconColor = computed(() => {
  return 'white';
});

function textToTestId(text: string): string {
  return text.toLowerCase().replace(/\s+/g, '-');
}
</script>

<style scoped>
.hero-stat-card {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  overflow: hidden;
  position: relative;
  cursor: default;
  min-height: 160px;
}

.hero-stat-card.clickable {
  cursor: pointer !important;
}

.hero-stat-card:hover {
  transform: translateY(-4px);
  box-shadow: 0 12px 28px rgba(0, 0, 0, 0.2) !important;
}

.hero-stat-card::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: radial-gradient(circle at top right, rgba(255, 255, 255, 0.1) 0%, transparent 60%);
  pointer-events: none;
}

.text-light-theme,
.text-dark-theme {
  color: white !important;
}

.text-light-theme .stat-label,
.text-dark-theme .stat-label {
  color: rgba(255, 255, 255, 0.9) !important;
}

.text-light-theme .stat-subtitle,
.text-dark-theme .stat-subtitle {
  color: rgba(255, 255, 255, 0.7) !important;
}

.stat-number {
  animation: countUp 0.6s ease-out forwards;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

.icon-avatar {
  backdrop-filter: blur(8px);
}

@keyframes countUp {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

:deep(.v-theme--dark) .hero-stat-card:hover {
  box-shadow: 0 12px 28px rgba(0, 0, 0, 0.5) !important;
}
</style>
