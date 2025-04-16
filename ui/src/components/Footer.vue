<template>
  <v-footer fixed class="footer-container py-1 px-2">
    <v-container fluid>
      <v-row align="center" justify="space-between">
        <!-- Brand and Version -->
        <v-col cols="12" sm="6" class="d-flex flex-column align-sm-start align-center mb-0">
          <div class="d-flex align-center">
            <v-icon icon="mdi-package-variant-closed" color="primary" class="me-2" size="small" />
            <span class="text-subtitle-2">kellnr</span>
          </div>
          <span class="text-caption text-medium-emphasis">v{{ version }}</span>
        </v-col>

        <!-- Links -->
        <v-col cols="12" sm="6" class="d-flex justify-center justify-sm-end mb-0">
          <v-btn-group variant="text" divided dense>
            <v-btn v-for="(link, index) in links" :key="index" :href="link.url" target="_blank"
              rel="noopener noreferrer" size="x-small" class="px-2" density="compact">
              <v-icon :icon="link.icon" size="small" class="me-1 d-none d-sm-inline" />
              <span class="d-none d-sm-inline">{{ link.text }}</span>
              <v-icon :icon="link.icon" size="small" class="d-sm-none" />
            </v-btn>
          </v-btn-group>
        </v-col>
      </v-row>
    </v-container>
  </v-footer>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import axios from "axios";
import { VERSION } from "../remote-routes";
import { useDisplay } from "vuetify";

// Get viewport size
const display = useDisplay();

// Version state
const version = ref("");

// Links configuration
const links = [
  {
    text: "kellnr.io",
    icon: "mdi-web",
    url: "https://kellnr.io/"
  },
  {
    text: "Docs",
    icon: "mdi-book-open-page-variant",
    url: "https://kellnr.io/documentation"
  },
  {
    text: "GitHub",
    icon: "mdi-github",
    url: "https://github.com/kellnr/kellnr"
  }
];

onMounted(() => {
  axios
    .get(VERSION)
    .then((res) => {
      version.value = res.data.version;
    })
    .catch((err) => {
      console.error("Failed to fetch version:", err);
    });
});
</script>

<style scoped>
.footer-container {
  border-top: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  position: sticky;
  bottom: 0;
  z-index: 5;
  min-height: 48px !important;
  max-height: 48px;
}
</style>
