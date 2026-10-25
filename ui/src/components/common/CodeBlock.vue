<template>
  <div class="code-block" data-testid="code-block">
    <pre v-highlightjs="code"><code :class="language"></code></pre>
    <v-tooltip :text="copyFeedback[copyState].text" location="top">
      <template v-slot:activator="{ props: tooltipProps }">
        <v-btn v-bind="tooltipProps" :icon="copyFeedback[copyState].icon" variant="text" size="small"
          :color="copyState === 'failed' ? 'error' : 'primary'" class="copy-btn" aria-label="Copy to clipboard"
          data-testid="code-block-copy" @click="copy" />
      </template>
    </v-tooltip>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';

const props = withDefaults(defineProps<{
  code: string;
  language?: string;
}>(), {
  language: 'bash',
});

type CopyState = 'idle' | 'copied' | 'failed';

const copyFeedback: Record<CopyState, { text: string; icon: string }> = {
  idle: { text: 'Copy to clipboard', icon: 'mdi-content-copy' },
  copied: { text: 'Copied', icon: 'mdi-check' },
  failed: { text: 'Copy failed', icon: 'mdi-alert-circle-outline' },
};

const copyState = ref<CopyState>('idle');

async function copy() {
  try {
    // The clipboard API only exists in secure contexts (HTTPS or localhost)
    await navigator.clipboard.writeText(props.code);
    copyState.value = 'copied';
  } catch (err) {
    console.error('Failed to copy code:', err);
    copyState.value = 'failed';
  }
  setTimeout(() => {
    copyState.value = 'idle';
  }, 2000);
}
</script>

<style scoped>
.code-block {
  position: relative;
  margin: 16px 0;
}

pre {
  margin: 0;
  padding: 16px 48px 16px 16px;
  overflow-x: auto;
  border-radius: 8px;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  font-size: 0.875rem;
  line-height: 1.6;
}

pre code {
  background: transparent;
  padding: 0;
}

.copy-btn {
  position: absolute;
  top: 8px;
  right: 8px;
}
</style>
