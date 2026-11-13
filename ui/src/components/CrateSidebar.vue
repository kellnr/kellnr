<template>
  <v-card elevation="0" class="sidebar-card" data-testid="crate-sidebar">
    <!-- Header -->
    <v-card-text class="pa-5">
      <!-- Install Section, the crate page shows it above the tabs on small screens -->
      <div v-if="showInstall">
        <CrateInstall :crate-name="crateName" :version="version" class="sidebar-section"
          @copy-to-clipboard="copyToClipboard" />

        <v-divider class="my-3"></v-divider>
      </div>

      <!-- Uploaded Section -->
      <div class="sidebar-section">
        <div class="d-flex align-center mb-2">
          <v-icon size="small" class="me-2 section-icon">mdi-calendar</v-icon>
          <span class="section-title">Published</span>
        </div>

        <div class="sidebar-content">
          <v-tooltip :text="exactDate(published)">
            <template v-slot:activator="{ props }">
              <span v-bind="props" data-testid="sidebar-published">{{ fromNow(published) }}</span>
            </template>
          </v-tooltip>
          <span v-if="publisher" :title="publisherToken ? `Published with the token ${publisherToken}` : undefined"
            data-testid="sidebar-publisher"> by {{ publisher }}</span>
        </div>
      </div>

      <!-- Downloads Section -->
      <div class="sidebar-section">
        <div class="d-flex align-center mb-2">
          <v-icon size="small" class="me-2 section-icon">mdi-download</v-icon>
          <span class="section-title">Downloads</span>
        </div>

        <div class="sidebar-content download-grid">
          <span class="download-label">Version:</span>
          <span class="download-number" data-testid="sidebar-version-downloads">{{ formatNumber(versionDownloads) }}</span>

          <span class="download-label">Total:</span>
          <span class="download-number" data-testid="sidebar-total-downloads">{{ formatNumber(totalDownloads) }}</span>
        </div>
      </div>

      <!-- Links Section -->
      <template v-if="repository || homepage || license">
        <v-divider class="my-3"></v-divider>

        <div v-if="repository" class="sidebar-section">
          <div class="d-flex align-center mb-2">
            <v-icon size="small" class="me-2 section-icon">mdi-source-repository</v-icon>
            <span class="section-title">Repository</span>
          </div>
          <div class="sidebar-content">
            <a :href="safeUrl(repository)" target="_blank" rel="noopener noreferrer" class="sidebar-link"
              data-testid="sidebar-repository">{{ displayUrl(repository) }}</a>
          </div>
        </div>

        <div v-if="homepage" class="sidebar-section">
          <div class="d-flex align-center mb-2">
            <v-icon size="small" class="me-2 section-icon">mdi-home-outline</v-icon>
            <span class="section-title">Homepage</span>
          </div>
          <div class="sidebar-content">
            <a :href="safeUrl(homepage)" target="_blank" rel="noopener noreferrer" class="sidebar-link"
              data-testid="sidebar-homepage">{{ displayUrl(homepage) }}</a>
          </div>
        </div>

        <div v-if="license" class="sidebar-section">
          <div class="d-flex align-center mb-2">
            <v-icon size="small" class="me-2 section-icon">mdi-scale-balance</v-icon>
            <span class="section-title">License</span>
          </div>
          <div class="sidebar-content" data-testid="sidebar-license">{{ license }}</div>
        </div>
      </template>

      <v-divider class="my-3"></v-divider>

      <!-- Documentation Section -->
      <div class="sidebar-section">
        <div class="d-flex align-center mb-2">
          <v-icon size="small" class="me-2 section-icon">mdi-book-open-variant</v-icon>
          <span class="section-title">Documentation</span>
        </div>

        <div class="sidebar-content docs-actions">
          <div v-if="documentationLink" @click="openDocsPage" class="d-flex align-center cursor-pointer text-primary"
            data-testid="sidebar-open-docs">
            <v-icon size="small" class="me-1">mdi-open-in-new</v-icon>
            Open documentation
          </div>
          <router-link v-else to="/publishdocs" class="d-flex align-center text-decoration-none text-primary"
            data-testid="sidebar-add-docs">
            <v-icon size="small" class="me-1">mdi-plus</v-icon>
            Add documentation
          </router-link>

          <v-btn v-if="canBuildDocs" color="primary" variant="outlined" height="36"
            prepend-icon="mdi-cog" @click="buildDocs" data-testid="sidebar-build-docs">
            {{ documentationLink ? 'Rebuild docs' : 'Build docs' }}
          </v-btn>
        </div>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import CrateInstall from "./crate/CrateInstall.vue";
import { formatNumber } from "../common/format";
import { safeUrl } from "../common/url";
import { useRelativeTime } from "../composables";

defineProps({
  crateName: {
    type: String,
    required: true
  },
  version: {
    type: String,
    required: true
  },
  // When the selected version was published, and by whom
  published: {
    type: String,
    required: true
  },
  publisher: {
    type: String,
    default: ''
  },
  publisherToken: {
    type: String,
    default: ''
  },
  versionDownloads: {
    type: Number,
    required: true
  },
  totalDownloads: {
    type: Number,
    required: true
  },
  documentationLink: {
    type: String,
    default: ''
  },
  canBuildDocs: {
    type: Boolean,
    default: false
  },
  repository: {
    type: String,
    default: ''
  },
  homepage: {
    type: String,
    default: ''
  },
  license: {
    type: String,
    default: ''
  },
  showInstall: {
    type: Boolean,
    default: true
  }
});

const emit = defineEmits<{
  'copy-to-clipboard': [text: string]
  'open-docs': []
  'build-docs': []
}>();

const { fromNow, exactDate } = useRelativeTime();

// Links are shown without the scheme, which only takes up space
function displayUrl(url: string): string {
  return url.replace(/^https?:\/\//, "").replace(/\/$/, "");
}

function copyToClipboard(text: string) {
  emit('copy-to-clipboard', text);
}

function openDocsPage() {
  emit('open-docs');
}

function buildDocs() {
  emit('build-docs');
}
</script>

<style scoped>
/* Enhanced Sidebar Styling */
.sidebar-card {
  border-radius: var(--kellnr-radius-card);
  overflow: hidden;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
}

.sidebar-section {
  margin-bottom: 16px;
}

.sidebar-content {
  padding-left: 28px;
  word-break: break-word;
  word-wrap: break-word;
  overflow-wrap: break-word;
  white-space: normal;
  color: rgb(var(--v-theme-on-surface));
}

.sidebar-link {
  color: rgb(var(--v-theme-primary));
  text-decoration: none;
  overflow-wrap: anywhere;
}

.sidebar-link:hover {
  text-decoration: underline;
}

/* Download display */
.download-grid {
  display: grid;
  grid-template-columns: auto 1fr;
  column-gap: 8px;
  row-gap: 4px;
}

.download-label {
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface-variant));
}

.download-number {
  font-weight: normal;
  word-break: break-all;
  overflow-wrap: break-word;
  color: rgb(var(--v-theme-on-surface));
}

.docs-actions {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
}

.cursor-pointer {
  cursor: pointer;
}

/* Section titles, the icons stay muted so color is left for links and actions */
.section-title {
  font-size: 0.875rem;
  font-weight: 600;
  color: rgb(var(--v-theme-on-surface));
}

.section-icon {
  color: rgb(var(--v-theme-on-surface-variant));
}
</style>
