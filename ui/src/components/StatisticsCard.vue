<template>
  <v-card class="statistics-card ma-2" :style="cardStyle" elevation="3" rounded="lg" height="120" :ripple="true">
    <v-card-text class="d-flex flex-column justify-space-between h-100 pa-4">
      <div class="d-flex justify-space-between align-center">
        <span class="text-h3 font-weight-bold">{{ num }}</span>
        <v-avatar :color="getIconBgColor" size="56" class="elevation-1">
          <v-icon :color="getIconColor" :icon="icon" size="large"></v-icon>
        </v-avatar>
      </div>
      <div class="text-subtitle-1 font-weight-medium mt-2">
        {{ text }}
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  num: number | string;
  icon: string; // MDI icon name (e.g., 'mdi-download', 'mdi-account-group')
  text: number | string;
  iconColor?: string;
  backgroundColor?: string;
  gradientColor?: string;
  category?: string;
}>();

// Compute card styles including background gradient
const cardStyle = computed(() => {
  if (props.backgroundColor) {
    if (props.gradientColor) {
      return {
        background: `linear-gradient(135deg, ${props.backgroundColor} 0%, ${props.gradientColor} 100%)`,
      };
    }
    return { backgroundColor: props.backgroundColor };
  }

  // Default styles based on category
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
  if (props.iconColor) {
    // For preset colors like #FFD700, return a matching light background
    if (props.iconColor === '#FFD700') return 'amber-lighten-4';
    if (props.iconColor === '#C0C0C0') return 'grey-lighten-3';
    if (props.iconColor === '#CD7F32') return 'orange-lighten-4';

    // For other color strings, make a best effort to return a light version
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
</style>
