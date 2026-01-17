<template>
  <v-card
    class="statistics-card ma-2"
    :style="cardStyle"
    elevation="3"
    rounded="lg"
    height="120"
    :ripple="true"
    :data-testid="`stat-card-${textToTestId(text)}`">
    <v-card-text class="d-flex flex-column justify-space-between h-100 pa-4" :class="textColorClass">
      <div class="d-flex justify-space-between align-center">
        <span class="text-h3 font-weight-bold" data-testid="stat-value">{{ num }}</span>
        <v-avatar :color="getIconBgColor" size="56" class="elevation-1">
          <v-icon :color="getIconColor" :icon="icon" size="large"></v-icon>
        </v-avatar>
      </div>
      <div class="text-subtitle-1 font-weight-medium mt-2" data-testid="stat-label">
        {{ text }}
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed, inject } from 'vue';
import { useTheme } from 'vuetify';

const props = defineProps<{
  num: number | string;
  icon: string; // MDI icon name (e.g., 'mdi-download', 'mdi-account-group')
  text: number | string;
  iconColor?: string;
  backgroundColor?: string;
  gradientColor?: string;
  category?: string;
}>();

// Get current theme
const theme = useTheme();
const isDark = computed(() => theme.global.current.value.dark);

// Text color class based on theme
const textColorClass = computed(() => {
  return isDark.value ? 'text-dark-theme' : '';
});

// Compute card styles including background gradient
const cardStyle = computed(() => {
  // Handle dark mode with different gradients
  if (isDark.value) {
    if (props.backgroundColor && props.gradientColor) {
      // Custom dark mode gradient for user-provided colors
      const darkBg = darkenColor(props.backgroundColor);
      const darkGradient = darkenColor(props.gradientColor);
      return {
        background: `linear-gradient(135deg, ${darkBg} 0%, ${darkGradient} 100%)`,
      };
    }

    // Default dark mode gradients based on category
    switch (props.category) {
      case 'primary':
        return { background: 'linear-gradient(135deg, #0d47a1 0%, #1565c0 100%)' };
      case 'secondary':
        return { background: 'linear-gradient(135deg, #4a148c 0%, #6a1b9a 100%)' };
      case 'gold':
        return { background: 'linear-gradient(135deg, #ff6f00 0%, #ff8f00 100%)' };
      case 'silver':
        return { background: 'linear-gradient(135deg, #424242 0%, #616161 100%)' };
      case 'bronze':
        return { background: 'linear-gradient(135deg, #bf360c 0%, #d84315 100%)' };
      case 'cached':
        return { background: 'linear-gradient(135deg, #1a237e 0%, #283593 100%)' };
      default:
        return { background: 'linear-gradient(135deg, #212121 0%, #424242 100%)' };
    }
  }

  // Original light mode styles
  if (props.backgroundColor) {
    if (props.gradientColor) {
      return {
        background: `linear-gradient(135deg, ${props.backgroundColor} 0%, ${props.gradientColor} 100%)`,
      };
    }
    return { backgroundColor: props.backgroundColor };
  }

  // Default light mode styles based on category
  switch (props.category) {
    case 'primary':
      return { background: 'linear-gradient(135deg, #e3f2fd 0%, #bbdefb 100%)' };
    case 'secondary':
      return { background: 'linear-gradient(135deg, #f3e5f5 0%, #e1bee7 100%)' };
    case 'gold':
      return { background: 'linear-gradient(135deg, #fff8e1 0%, #ffe082 100%)' };
    case 'silver':
      return { background: 'linear-gradient(135deg, #f5f5f5 0%, #e0e0e0 100%)' };
    case 'bronze':
      return { background: 'linear-gradient(135deg, #fff3e0 0%, #ffcc80 100%)' };
    case 'cached':
      return { background: 'linear-gradient(135deg, #e8eaf6 0%, #c5cae9 100%)' };
    default:
      return { background: 'linear-gradient(135deg, #f5f5f5 0%, #e0e0e0 100%)' };
  }
});

// Determine icon background color based on iconColor or category
const getIconBgColor = computed(() => {
  // Dark theme adjustments for icon background
  if (isDark.value) {
    if (props.iconColor) {
      // For preset colors in dark mode
      if (props.iconColor === '#FFD700') return 'amber-darken-2';
      if (props.iconColor === '#C0C0C0') return 'grey-darken-1';
      if (props.iconColor === '#CD7F32') return 'orange-darken-2';
      return 'grey-darken-3';
    }

    switch (props.category) {
      case 'primary': return 'blue-darken-4';
      case 'secondary': return 'purple-darken-4';
      case 'gold': return 'amber-darken-4';
      case 'silver': return 'grey-darken-3';
      case 'bronze': return 'orange-darken-4';
      case 'cached': return 'indigo-darken-4';
      default: return 'grey-darken-3';
    }
  }

  // Original light mode icon backgrounds
  if (props.iconColor) {
    if (props.iconColor === '#FFD700') return 'amber-lighten-4';
    if (props.iconColor === '#C0C0C0') return 'grey-lighten-3';
    if (props.iconColor === '#CD7F32') return 'orange-lighten-4';
    return 'white';
  }

  switch (props.category) {
    case 'primary': return 'blue-lighten-5';
    case 'secondary': return 'purple-lighten-5';
    case 'gold': return 'amber-lighten-5';
    case 'silver': return 'grey-lighten-4';
    case 'bronze': return 'orange-lighten-5';
    case 'cached': return 'indigo-lighten-5';
    default: return 'grey-lighten-4';
  }
});

// Determine icon color
const getIconColor = computed(() => {
  // Dark theme icon colors
  if (isDark.value) {
    if (props.iconColor) {
      // Lighten preset colors for dark mode
      if (props.iconColor === '#FFD700') return 'amber-lighten-1';
      if (props.iconColor === '#C0C0C0') return 'grey-lighten-1';
      if (props.iconColor === '#CD7F32') return 'orange-lighten-1';
      return props.iconColor;
    }

    switch (props.category) {
      case 'primary': return 'blue-lighten-2';
      case 'secondary': return 'purple-lighten-2';
      case 'gold': return 'amber-lighten-1';
      case 'silver': return 'grey-lighten-1';
      case 'bronze': return 'orange-lighten-1';
      case 'cached': return 'indigo-lighten-2';
      default: return 'grey-lighten-1';
    }
  }

  // Original light mode icon colors
  if (props.iconColor) return props.iconColor;

  switch (props.category) {
    case 'primary': return 'primary';
    case 'secondary': return 'secondary';
    case 'gold': return 'amber-darken-2';
    case 'silver': return 'grey-darken-1';
    case 'bronze': return 'orange-darken-1';
    case 'cached': return 'indigo';
    default: return 'grey-darken-1';
  }
});

// Helper function to darken a color for dark mode
function darkenColor(color: string): string {
  // Simple implementation - in a real app, you might want a more sophisticated color transformation
  if (color.startsWith('#')) {
    // Convert hex to darker version
    return color.replace(/^#/, '#0');
  }
  return color;
}

// Helper function to convert text to test ID format
function textToTestId(text: number | string): string {
  return String(text).toLowerCase().replace(/\s+/g, '-');
}
</script>

<style scoped>
.statistics-card {
  transition: all 0.3s ease;
  overflow: hidden;
  position: relative;
}

.statistics-card:hover {
  transform: translateY(-3px);
  box-shadow: 0 8px 16px rgba(0, 0, 0, 0.1) !important;
}

/* Text color for dark theme */
.text-dark-theme {
  color: rgba(255, 255, 255, 0.9) !important;
}

.text-dark-theme .text-subtitle-1 {
  color: rgba(255, 255, 255, 0.7) !important;
}

/* Animation for numbers */
@keyframes countUp {
  from {
    opacity: 0;
    transform: translateY(15px);
  }

  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.text-h3 {
  animation: countUp 0.5s ease-out forwards;
}

/* Dark mode shadow adjustment */
:deep(.v-theme--dark) .statistics-card:hover {
  box-shadow: 0 8px 16px rgba(0, 0, 0, 0.4) !important;
}
</style>
