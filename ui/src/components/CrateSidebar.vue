<template>
  <v-card elevation="1" class="sidebar-card">
    <!-- Header -->
    <v-card-title class="sidebar-header">
      <v-icon class="me-2">mdi-package-variant</v-icon>
      Crate Information
    </v-card-title>

    <v-card-text class="pt-0">
      <!-- Install Section -->
      <div class="sidebar-section" data-testid="install-section">
        <div class="d-flex align-center mb-2">
          <v-icon color="primary" size="small" class="me-2">mdi-code-braces</v-icon>
          <span class="text-subtitle-1 font-weight-medium">Install</span>
        </div>

        <div class="d-flex align-center copy-container mt-1">
          <div class="copy-text text-body-2" data-testid="install-snippet">{{ crateName }} = "{{ version }}"</div>
          <v-tooltip text="Copy to clipboard">
            <template v-slot:activator="{ props }">
              <v-btn v-bind="props" icon variant="text" density="comfortable" color="primary"
                @click="copyTomlToClipboard()" class="copy-btn ml-auto">
                <v-icon size="small">mdi-content-copy</v-icon>
              </v-btn>
            </template>
          </v-tooltip>
        </div>
      </div>

      <v-divider class="my-3"></v-divider>

      <!-- Version Information -->
      <div class="sidebar-section">
        <div class="d-flex align-center mb-2">
          <v-icon color="info" size="small" class="me-2">mdi-tag-outline</v-icon>
          <span class="text-subtitle-1 font-weight-medium">Version</span>
        </div>

        <div class="sidebar-content">
          {{ version }}
        </div>
      </div>

      <!-- Uploaded Section -->
      <div class="sidebar-section">
        <div class="d-flex align-center mb-2">
          <v-icon color="info" size="small" class="me-2">mdi-calendar</v-icon>
          <span class="text-subtitle-1 font-weight-medium">Published</span>
        </div>

        <div class="sidebar-content">
          <v-tooltip :text="lastUpdated">
            <template v-slot:activator="{ props }">
              <span v-bind="props">{{ humanizedLastUpdated }}</span>
            </template>
          </v-tooltip>
        </div>
      </div>

      <!-- Downloads Section -->
      <div class="sidebar-section">
        <div class="d-flex align-center mb-2">
          <v-icon color="success" size="small" class="me-2">mdi-download</v-icon>
          <span class="text-subtitle-1 font-weight-medium">Downloads</span>
        </div>

        <div class="sidebar-content">
          <div class="download-item">
            <span class="download-label">Version:</span>
            <span class="download-number">{{ versionDownloads.toLocaleString() }}</span>
          </div>

          <div class="download-item mt-1">
            <span class="download-label">Total:</span>
            <span class="download-number">{{ totalDownloads.toLocaleString() }}</span>
          </div>
        </div>
      </div>

      <v-divider class="my-3"></v-divider>

      <!-- Documentation Section -->
      <div class="sidebar-section">
        <div class="d-flex align-center mb-2">
          <v-icon color="warning" size="small" class="me-2">mdi-book-open-variant</v-icon>
          <span class="text-subtitle-1 font-weight-medium">Documentation</span>
        </div>

        <div class="sidebar-content">
          <div v-if="documentationLink" @click="openDocsPage" class="cursor-pointer text-primary">
            <v-icon size="small" class="me-1">mdi-open-in-new</v-icon>
            Open documentation
          </div>
          <router-link v-else to="/publishdocs" class="text-decoration-none">
            <v-icon size="small" class="me-1">mdi-plus</v-icon>
            Add documentation
          </router-link>

          <v-btn v-if="canBuildDocs" color="primary" variant="outlined" size="small" density="comfortable"
            prepend-icon="mdi-cog" @click="buildDocs" class="mt-2">
            {{ documentationLink ? 're-build docs' : 'build docs' }}
          </v-btn>
        </div>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
defineProps({
  crateName: {
    type: String,
    required: true
  },
  version: {
    type: String,
    required: true
  },
  lastUpdated: {
    type: String,
    required: true
  },
  humanizedLastUpdated: {
    type: String,
    required: true
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
  }
});

const emit = defineEmits(['copy-to-clipboard', 'open-docs', 'build-docs']);

function copyTomlToClipboard() {
  emit('copy-to-clipboard');
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
  border-radius: 12px;
  overflow: hidden;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
}

.sidebar-header {
  background: rgb(var(--v-theme-surface-variant));
  color: rgb(var(--v-theme-on-surface));
  padding-top: 16px;
  padding-bottom: 16px;
  font-weight: 500;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
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
  border-radius: 6px;
  min-height: 36px;
  border: 1px solid rgb(var(--v-theme-outline));
  width: 100%;
}

.copy-text {
  white-space: normal;
  word-break: break-all;
  overflow-wrap: break-word;
  max-width: calc(100% - 36px);
  padding-right: 8px;
  font-family: 'Roboto Mono', monospace;
  color: rgb(var(--v-theme-on-surface));
}

.copy-btn {
  opacity: 0.7;
  transition: opacity 0.2s;
}

.copy-container:hover .copy-btn {
  opacity: 1;
}

/* Download display */
.download-item {
  display: flex;
  align-items: flex-start;
}

.download-label {
  min-width: 60px;
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface-variant));
}

.download-number {
  font-weight: normal;
  word-break: break-all;
  overflow-wrap: break-word;
  color: rgb(var(--v-theme-on-surface));
}

.cursor-pointer {
  cursor: pointer;
}

/* Section titles */
.text-subtitle-1 {
  color: rgb(var(--v-theme-on-surface));
}
</style>
