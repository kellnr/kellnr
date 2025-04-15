<template>
  <v-app :theme="store.theme">
    <!-- Header with full width -->
    <Header />

    <!-- Main content with responsive width -->
    <v-main>
      <!-- Container with responsive width classes -->
      <v-container :class="{
        'width-80': $vuetify.display.mdAndUp,
        'mx-auto': $vuetify.display.mdAndUp
      }" class="px-0" fluid>
        <router-view></router-view>
      </v-container>
    </v-main>

    <!-- Footer with full width -->
    <Footer />
  </v-app>
</template>

<script setup lang="ts">
import { useStore } from './store/store';
import { onMounted } from 'vue';
import Header from './components/Header.vue';
import Footer from './components/Footer.vue';
import { useTheme, useDisplay } from 'vuetify';

const store = useStore();
const vuetifyTheme = useTheme();
const display = useDisplay();

onMounted(() => {
  // Set Vuetify theme based on store
  vuetifyTheme.global.name.value = store.theme;

  // Initialize highlight.js theme based on current theme
  const isDark = store.theme === 'dark';
  const hlLight = document.querySelector('link[href*="highlight.js/styles/github.css"]');
  const hlDark = document.querySelector('link[href*="highlight.js/styles/vs2015.css"]');

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

<style>
/* Custom class for 80% width */
.width-80 {
  width: 80% !important;
  max-width: 1440px !important;
  /* Optional: set a maximum width */
}

/* Remove padding on small screens for full width content */
@media (max-width: 959px) {
  .v-container {
    padding-left: 0 !important;
    padding-right: 0 !important;
  }
}
</style>
