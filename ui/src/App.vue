<template>
  <v-app :theme="store.theme">
    <!-- Header with full width -->
    <Header />

    <!-- Main content, all pages share the same width -->
    <v-main>
      <v-container class="page-container">
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
import { useTheme } from 'vuetify';

const store = useStore();
const vuetifyTheme = useTheme();

onMounted(() => {
  // Set Vuetify theme based on store
  vuetifyTheme.global.name.value = store.theme;

  // Set body attribute for custom CSS
  let body = document.getElementById("body");
  body?.setAttribute("color-theme", store.theme);
});
</script>

<style>
:root {
  --kellnr-font-sans: 'Inter Variable', system-ui, -apple-system, 'Segoe UI', sans-serif;
  --kellnr-font-mono: 'JetBrains Mono Variable', ui-monospace, 'SF Mono', Consolas, monospace;
  /* Vuetify reads its fonts from these */
  --v-font-body: var(--kellnr-font-sans);
  --v-font-heading: var(--kellnr-font-sans);

  /* Corner radii: one for cards and dialogs, one for everything inside them */
  --kellnr-radius-card: 12px;
  --kellnr-radius: 8px;

  /* Width of the page content */
  --kellnr-page-width: 1200px;
}

/* Always reserve the scrollbar's space, so the layout does not shift
   sideways when a page becomes long enough to scroll */
html {
  scrollbar-gutter: stable;
  background: rgb(var(--v-theme-background));
}

body {
  font-family: var(--kellnr-font-sans);
  -webkit-font-smoothing: antialiased;
}

code,
kbd,
pre,
samp {
  font-family: var(--kellnr-font-mono);
}

/* Counts, sizes and versions line up when their digits have the same width */
.tabular-nums {
  font-variant-numeric: tabular-nums;
}

/* Used by cards through the rounded="card" default */
.rounded-card {
  border-radius: var(--kellnr-radius-card);
}

.page-container {
  max-width: var(--kellnr-page-width);
  padding-top: 32px;
  padding-bottom: 48px;
}

@media (max-width: 600px) {
  .page-container {
    padding-top: 16px;
    padding-bottom: 32px;
  }
}
</style>
