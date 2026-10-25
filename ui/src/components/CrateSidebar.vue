<template>
  <v-card elevation="0" class="sidebar-card" data-testid="crate-sidebar">
    <!-- Header -->
    <v-card-text class="pa-5">
      <!-- Install Section -->
      <div class="sidebar-section" data-testid="install-section">
        <div class="d-flex align-center mb-2">
          <v-icon size="small" class="me-2 section-icon">mdi-code-braces</v-icon>
          <span class="section-title">Install</span>
        </div>

        <div v-for="snippet in installSnippets" :key="snippet.testId" class="snippet">
          <div class="text-body-small snippet-label">{{ snippet.label }}</div>
          <div class="d-flex align-center copy-container mt-1">
            <!-- Lines break only between words, never inside a name or version -->
            <div class="copy-text text-body-medium" :data-testid="snippet.testId">
              <template v-for="(word, i) in snippet.text.split(' ')" :key="i">{{ i > 0 ? ' ' : '' }}<span
                  class="word">{{ word }}</span></template>
            </div>
            <v-tooltip text="Copy to clipboard">
              <template v-slot:activator="{ props }">
                <v-btn v-bind="props" icon variant="text" density="comfortable" color="primary"
                  :aria-label="`Copy ${snippet.label}`" @click="copyToClipboard(snippet.text)"
                  class="copy-btn ml-auto" :data-testid="snippet.copyTestId">
                  <v-icon size="small">mdi-content-copy</v-icon>
                </v-btn>
              </template>
            </v-tooltip>
          </div>
        </div>
      </div>

      <v-divider class="my-3"></v-divider>

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

          <v-btn v-if="canBuildDocs" color="primary" variant="outlined" size="small" density="comfortable"
            prepend-icon="mdi-cog" @click="buildDocs" data-testid="sidebar-build-docs">
            {{ documentationLink ? 'Rebuild docs' : 'Build docs' }}
          </v-btn>
        </div>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { formatNumber } from "../common/format";
import { cargoAddCommand, cargoTomlDependency } from "../common/registry";
import { safeUrl } from "../common/url";
import { useRelativeTime } from "../composables";

const props = defineProps({
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
  }
});

const emit = defineEmits<{
  'copy-to-clipboard': [text: string]
  'open-docs': []
  'build-docs': []
}>();

const { fromNow, exactDate } = useRelativeTime();

// Crates on this page are hosted in Kellnr, so the snippets name the registry.
// Without it, cargo looks the crate up on crates.io.
const installSnippets = computed(() => [
  {
    label: "Cargo.toml",
    text: cargoTomlDependency(props.crateName, props.version),
    testId: "install-snippet",
    copyTestId: "sidebar-copy-button",
  },
  {
    label: "Command line",
    text: cargoAddCommand(props.crateName),
    testId: "install-command",
    copyTestId: "sidebar-copy-command-button",
  },
]);

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

/* Copy button and container styling */
.copy-container {
  position: relative;
  padding: 8px 12px;
  background: rgb(var(--v-theme-surface-variant));
  border-radius: var(--kellnr-radius);
  min-height: 36px;
  border: 1px solid rgb(var(--v-theme-outline));
  width: 100%;
}

.copy-text {
  white-space: normal;
  min-width: 0;
  flex: 1;
  padding-right: 8px;
  font-family: var(--kellnr-font-mono);
  color: rgb(var(--v-theme-on-surface));
}

.word {
  white-space: nowrap;
}

.snippet + .snippet {
  margin-top: 12px;
}

.snippet-label {
  color: rgb(var(--v-theme-on-surface-variant));
}

.sidebar-link {
  color: rgb(var(--v-theme-primary));
  text-decoration: none;
  overflow-wrap: anywhere;
}

.sidebar-link:hover {
  text-decoration: underline;
}

.copy-btn {
  opacity: 0.7;
  transition: opacity 0.2s;
}

.copy-container:hover .copy-btn {
  opacity: 1;
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
