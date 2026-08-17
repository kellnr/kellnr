<template>
  <v-btn icon variant="text" @click="toggleTheme" class="mr-2" data-testid="theme-toggle"
    :title="isDark ? 'Switch to light mode' : 'Switch to dark mode'">
    <v-icon>{{ isDark ? 'mdi-white-balance-sunny' : 'mdi-moon-waxing-crescent' }}</v-icon>
  </v-btn>
</template>

<script setup lang="ts">
import { useStore } from '../store/store';
import { computed } from 'vue';
import { useTheme } from 'vuetify';

const store = useStore();
const vuetifyTheme = useTheme();
const isDark = computed(() => store.theme === 'dark');

function toggleTheme() {
  // Toggle theme in store
  store.toggleTheme();

  // Update Vuetify theme
  vuetifyTheme.global.name.value = store.theme;

  // Update body attribute
  let body = document.getElementById("body");
  body?.setAttribute("color-theme", store.theme);

  // Highlight.js theme follows the Vuetify theme reactively in Readme.vue.
}
</script>
