<template>
  <div class="install-section" data-testid="install-section">
    <div class="d-flex align-center mb-2">
      <v-icon size="small" class="me-2 section-icon">mdi-code-braces</v-icon>
      <span class="section-title">Install</span>
    </div>

    <div v-for="snippet in installSnippets" :key="snippet.testId" class="snippet">
      <div class="text-body-small snippet-label">{{ snippet.label }}</div>
      <div class="d-flex align-center copy-container mt-1">
        <!-- Snippets never wrap, a long one scrolls sideways instead -->
        <div class="copy-text text-body-medium" :data-testid="snippet.testId">
          <template v-for="(word, i) in snippet.text.split(' ')" :key="i">{{ i > 0 ? ' ' : '' }}<span
              class="word">{{ word }}</span></template>
        </div>
        <v-tooltip text="Copy to clipboard">
          <template v-slot:activator="{ props: tooltipProps }">
            <v-btn v-bind="tooltipProps" icon variant="text" density="comfortable" color="primary"
              :aria-label="`Copy ${snippet.label}`" @click="emit('copy-to-clipboard', snippet.text)"
              class="copy-btn ml-auto" :data-testid="snippet.copyTestId">
              <v-icon size="small">mdi-content-copy</v-icon>
            </v-btn>
          </template>
        </v-tooltip>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { cargoAddCommand, cargoTomlDependency } from "../../common/registry";

const props = defineProps<{
  crateName: string
  version: string
}>();

const emit = defineEmits<{
  'copy-to-clipboard': [text: string]
}>();

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
</script>

<style scoped>
.copy-container {
  position: relative;
  padding: 4px 4px 4px 12px;
  background: rgb(var(--v-theme-surface-variant));
  border-radius: var(--kellnr-radius);
  min-height: 36px;
  border: 1px solid rgb(var(--v-theme-outline));
  width: 100%;
}

.copy-text {
  white-space: pre;
  overflow-x: auto;
  min-width: 0;
  flex: 1;
  padding: 4px 8px 4px 0;
  font-family: var(--kellnr-font-mono);
  color: rgb(var(--v-theme-on-surface));
}

.snippet + .snippet {
  margin-top: 12px;
}

.snippet-label {
  color: rgb(var(--v-theme-on-surface-variant));
}

.copy-btn {
  flex-shrink: 0;
  opacity: 0.7;
  transition: opacity 0.2s;
}

.copy-container:hover .copy-btn,
.copy-btn:focus-visible {
  opacity: 1;
}

.section-title {
  font-size: 0.875rem;
  font-weight: 600;
  color: rgb(var(--v-theme-on-surface));
}

.section-icon {
  color: rgb(var(--v-theme-on-surface-variant));
}
</style>
