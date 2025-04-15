<template>
  <v-app :theme="store.theme">
    <Header />
    <v-main>
      <router-view></router-view>
    </v-main>
    <Footer />
  </v-app>
</template>

<script setup lang="ts">
import { useStore } from './store/store';
import { onMounted } from 'vue';
import Header from './components/Header.vue';
import Footer from './components/Footer.vue';
import { useTheme } from 'vuetify';

const store = useStore();
const vuetifyTheme = useTheme();

onMounted(() => {
  // Set Vuetify theme based on store
  vuetifyTheme.global.name.value = store.theme;

  // Initialize highlight.js theme based on current theme
  const isDark = store.theme === 'dark';
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

  // Set body attribute for custom CSS
  let body = document.getElementById("body");
  body?.setAttribute("color-theme", store.theme);
});
</script>
