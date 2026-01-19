<template>
  <v-card
    class="recent-crate-card clickable"
    :style="cardStyle"
    elevation="3"
    rounded="xl"
    ripple
    data-testid="recent-crate-card"
    @click="handleClick">
    <v-card-text class="d-flex align-center h-100 pa-4" :class="textColorClass">
      <v-avatar :color="iconBgColor" size="48" class="mr-4 elevation-1">
        <v-icon color="white" icon="mdi-package-variant" size="large"></v-icon>
      </v-avatar>
      <div class="flex-grow-1 overflow-hidden">
        <div class="text-overline mb-1 label-text">Last Updated</div>
        <div class="text-h6 font-weight-bold crate-name text-truncate" :title="crateName">
          {{ crateName }}
        </div>
        <div class="text-caption time-text">{{ timeAgo }}</div>
      </div>
      <v-icon :color="chevronColor" icon="mdi-chevron-right" size="small" class="ml-2"></v-icon>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useTheme } from 'vuetify';

const props = defineProps<{
  crateName: string;
  timeAgo: string;
  onClick?: () => void;
}>();

function handleClick() {
  if (props.onClick) {
    props.onClick();
  }
}

const theme = useTheme();
const isDark = computed(() => theme.global.current.value.dark);

const textColorClass = computed(() => {
  return isDark.value ? 'text-dark-theme' : 'text-light-theme';
});

const cardStyle = computed(() => {
  if (isDark.value) {
    return {
      background: '#1B2838',
      border: '1px solid #3D5068',
      borderLeft: '4px solid #64B5F6'
    };
  }
  return {
    background: '#FFFFFF',
    border: '1px solid #D0D7DE',
    borderLeft: '4px solid #1976D2'
  };
});

const iconBgColor = computed(() => {
  return isDark.value ? 'primary' : 'primary';
});

const chevronColor = computed(() => {
  return isDark.value ? 'grey-lighten-1' : 'grey-darken-1';
});
</script>

<style scoped>
.recent-crate-card {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  overflow: hidden;
  cursor: pointer;
  min-height: 90px;
}

.recent-crate-card:hover {
  transform: translateX(4px);
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.12) !important;
}

.text-light-theme .label-text {
  color: rgba(0, 0, 0, 0.6) !important;
}

.text-light-theme .crate-name {
  color: rgba(0, 0, 0, 0.87) !important;
}

.text-light-theme .time-text {
  color: rgba(0, 0, 0, 0.5) !important;
}

.text-dark-theme .label-text {
  color: rgba(255, 255, 255, 0.6) !important;
}

.text-dark-theme .crate-name {
  color: rgba(255, 255, 255, 0.95) !important;
}

.text-dark-theme .time-text {
  color: rgba(255, 255, 255, 0.5) !important;
}

:deep(.v-theme--dark) .recent-crate-card:hover {
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.4) !important;
}
</style>
