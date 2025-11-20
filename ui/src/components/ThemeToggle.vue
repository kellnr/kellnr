<template>
  <v-btn icon variant="text" @click="toggleTheme" class="mr-2"
    :title="isDark ? 'Switch to light mode' : 'Switch to dark mode'">
    <v-icon>{{ isDark ? 'mdi-white-balance-sunny' : 'mdi-moon-waxing-crescent' }}</v-icon>
  </v-btn>
</template>

<script setup lang="ts">
import { useStore } from '../store/store';
import { storeToRefs } from 'pinia';
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

  // Toggle highlight.js theme
  toggleHighlightTheme(store.theme);
}

function toggleHighlightTheme(theme: string) {
  const isDark = theme === 'dark';

  // Select all highlight.js style links
  const hlLight = document.querySelector('link[href*="highlight.js/styles/github.css"]');
  const hlDark = document.querySelector('link[href*="highlight.js/styles/github-dark.css"]');

  if (hlLight && hlDark) {
    if (isDark) {
      hlLight.setAttribute('disabled', 'true');
      hlDark.removeAttribute('disabled');
    } else {
      hlLight.removeAttribute('disabled');
      hlDark.setAttribute('disabled', 'true');
    }
  }
}
</script>
