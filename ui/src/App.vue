<template>
  <v-app :theme="store.theme">
    <div class="bg-image" :style="{ backgroundImage: `url(${bgImage})` }"></div>
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
import { onMounted, computed, watch, onBeforeMount } from 'vue';
import Header from './components/Header.vue';
import Footer from './components/Footer.vue';
import { useTheme } from 'vuetify';

const store = useStore();
const vuetifyTheme = useTheme();

// Create a direct reference to the background image URL
const bgImage = computed(() => {
  return store.currentBackgroundImage;
});

// Update background when theme changes
watch(() => store.theme, () => {
}, { immediate: true });

onBeforeMount(() => {
  // Make sure store is properly initialized with the correct initial background
  if (store.theme === 'dark') {
    store.currentBackgroundImage = store.darkBackgroundImage;
  } else {
    store.currentBackgroundImage = store.lightBackgroundImage;
  }
});

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

/* Ensure main content has space for footer and stacks above bg */
.v-main {
  padding-bottom: 64px !important;
  position: relative;
  z-index: 1;
}

:root {
  --bg-image: url('');
}

.bg-image {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-image: var(--bg-image);
  background-size: cover;
  background-position: center;
  background-repeat: no-repeat;
  z-index: 0;
  /* Place it behind other content */
  opacity: 1;
  /* Adjust opacity as needed */
}
</style>
