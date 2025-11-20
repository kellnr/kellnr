<template>
  <v-footer fixed class="footer-container py-1 px-2">
    <div class="d-flex align-center justify-space-between w-100">
      <!-- Brand and Version - Always Inline -->
      <div class="d-flex align-center">
        <v-icon icon="mdi-package-variant-closed" color="primary" class="me-1" size="small" />
        <span class="text-subtitle-2 me-1">kellnr</span>
        <span class="text-caption text-medium-emphasis">v{{ version }}</span>
      </div>

      <!-- Links - Always Horizontal -->
      <div>
        <v-btn-group variant="text" density="compact">
          <v-btn v-for="(link, index) in links" :key="index" :href="link.url" target="_blank" rel="noopener noreferrer"
            size="x-small" density="comfortable" class="link-btn">
            <v-icon :icon="link.icon" size="small" />
            <span class="link-text">{{ link.text }}</span>
          </v-btn>
        </v-btn-group>
      </div>
    </div>
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
  overflow: hidden;
  background-color: var(--v-theme-surface);
}

.link-btn {
  padding: 0 8px !important;
}

.link-text {
  display: none;
  margin-left: 4px;
}

/* Show text labels on medium screens and up */
@media (min-width: 600px) {
  .link-text {
    display: inline;
  }

  .link-btn {
    padding: 0 12px !important;
  }
}
</style>
