<template>
  <v-footer app class="footer-container">
    <div class="footer-content">
      <!-- Brand and Version -->
      <div class="brand-section">
        <v-icon icon="mdi-package-variant-closed" class="brand-icon" size="small" />
        <span class="brand-name">
          <span class="brand-bracket">&lt;</span><span class="brand-lifetime">'</span><span class="brand-kellnr">k</span><span class="brand-bracket">&gt;</span>
        </span>
        <span class="version-text">v{{ version }}</span>
      </div>

      <!-- Links -->
      <div class="links-section">
        <a
          v-for="(link, index) in links"
          :key="index"
          :href="link.url"
          target="_blank"
          rel="noopener noreferrer"
          class="footer-link"
        >
          <v-icon :icon="link.icon" size="small" />
          <span class="link-text">{{ link.text }}</span>
        </a>
      </div>
    </div>
  </v-footer>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { settingsService } from "../services";
import { isSuccess } from "../services/api";

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

onMounted(async () => {
  const result = await settingsService.getVersion();
  if (isSuccess(result)) {
    version.value = result.data.version;
  }
});
</script>

<style scoped>
.footer-container {
  background: rgba(var(--v-theme-surface), 0.85) !important;
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border-top: 1px solid rgba(var(--v-theme-outline), 0.5);
  z-index: 5;
  min-height: 48px !important;
  max-height: 48px;
  padding: 0 16px;
}

.footer-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  height: 100%;
}

.brand-section {
  display: flex;
  align-items: center;
  gap: 6px;
}

.brand-icon {
  color: rgb(var(--v-theme-primary));
}

.brand-name {
  font-size: 0.875rem;
  font-weight: 600;
  font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', 'Consolas', monospace;
  display: inline-flex;
  align-items: center;
}

.brand-bracket {
  color: rgb(var(--v-theme-on-surface-variant));
  font-weight: 500;
  opacity: 0.7;
}

.brand-lifetime {
  color: rgb(var(--v-theme-on-surface-variant));
  font-weight: 600;
  opacity: 0.8;
}

.brand-kellnr {
  color: rgb(var(--v-theme-primary));
  font-weight: 700;
}

.version-text {
  font-size: 0.75rem;
  color: rgb(var(--v-theme-on-surface-variant));
  padding: 2px 6px;
  background: rgba(var(--v-theme-primary), 0.08);
  border-radius: 4px;
}

.links-section {
  display: flex;
  align-items: center;
  gap: 4px;
}

.footer-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  color: rgb(var(--v-theme-on-surface-variant));
  text-decoration: none;
  font-size: 0.8rem;
  font-weight: 500;
  border-radius: 6px;
  transition: all 0.2s ease;
}

.footer-link:hover {
  background: rgba(var(--v-theme-primary), 0.08);
  color: rgb(var(--v-theme-primary));
}

.footer-link .v-icon {
  font-size: 16px;
  opacity: 0.8;
}

.footer-link:hover .v-icon {
  opacity: 1;
}

.link-text {
  display: none;
}

/* Show text labels on medium screens and up */
@media (min-width: 600px) {
  .link-text {
    display: inline;
  }
}

@media (max-width: 600px) {
  .footer-container {
    padding: 0 12px;
  }

  .footer-link {
    padding: 6px 8px;
  }
}
</style>
