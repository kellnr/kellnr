<template>
  <div class="readme-wrapper">
    <div class="markdown-body" data-testid="crate-readme" v-html="markedReadme"></div>
  </div>
</template>

<script setup lang="ts">
import DOMPurify from 'dompurify';
import { README_SANITIZE_CONFIG } from '../common/sanitize';
import { renderReadme } from '../common/readme';
import { computed, watchEffect } from "vue";
import githubLight from '@/assets/hljs-github.css?inline';
import githubDark from '@/assets/hljs-github-dark.css?inline';
import { useTheme } from 'vuetify';

const theme = useTheme();

const props = defineProps<{
  readme: string | null
}>();

const markedReadme = computed(() => {
  return DOMPurify.sanitize(renderReadme(props.readme || ''), README_SANITIZE_CONFIG);
});

// Switch the highlight.js theme with the Vuetify theme, using locally bundled
// stylesheets so no request is made to an external CDN (works offline /
// air-gapped and keeps a strict CSP happy).
watchEffect(() => {
  const styleId = 'highlight-theme';
  let style = document.getElementById(styleId) as HTMLStyleElement | null;

  if (!style) {
    style = document.createElement('style');
    style.id = styleId;
    document.head.appendChild(style);
  }

  style.textContent = theme.global.current.value.dark ? githubDark : githubLight;
});
</script>

<style scoped>
.readme-wrapper {
  padding: 24px;
  color: rgb(var(--v-theme-on-surface));
}

.markdown-body {
  font-family: inherit;
  color: rgb(var(--v-theme-on-surface));
}

.markdown-body :deep(pre) {
  border-radius: var(--kellnr-radius);
  margin: 16px 0;
  padding: 16px;
  overflow-x: auto;
  font-family: var(--kellnr-font-mono);
  font-size: 14px;
  line-height: 1.6;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline-variant));
}

/* The pre is the code box, the highlight.js theme only colors the tokens */
.markdown-body :deep(pre code),
.markdown-body :deep(pre code.hljs) {
  display: block;
  padding: 0;
  overflow-x: visible;
  font: inherit;
  color: rgb(var(--v-theme-on-surface));
  background: transparent;
}

.markdown-body :deep(code:not(pre code)) {
  color: inherit;
  padding: 0.2em 0.4em;
  border-radius: var(--kellnr-radius);
  font-size: 0.9em;
  background: rgb(var(--v-theme-surface-variant));
}

.markdown-body :deep(h1),
.markdown-body :deep(h2),
.markdown-body :deep(h3),
.markdown-body :deep(h4),
.markdown-body :deep(h5),
.markdown-body :deep(h6) {
  margin-top: 24px;
  margin-bottom: 16px;
  font-weight: 600;
  line-height: 1.25;
  color: rgb(var(--v-theme-on-surface));
}

.markdown-body :deep(h2) {
  font-size: 1.6em;
}

.markdown-body :deep(h3) {
  font-size: 1.3em;
}

.markdown-body :deep(h4) {
  font-size: 1.125em;
}

.markdown-body :deep(h5),
.markdown-body :deep(h6) {
  font-size: 1em;
}

.markdown-body :deep(p) {
  margin-bottom: 16px;
}

.markdown-body :deep(img) {
  max-width: 100%;
  height: auto;
}

.markdown-body :deep(table) {
  border-collapse: collapse;
  width: 100%;
  margin-bottom: 16px;
}

.markdown-body :deep(table th),
.markdown-body :deep(table td) {
  border: 1px solid rgb(var(--v-theme-outline));
  padding: 6px 13px;
}

.markdown-body :deep(a) {
  color: rgb(var(--v-theme-primary));
}

.markdown-body :deep(blockquote) {
  margin: 16px 0;
  padding: 0 16px;
  border-left: 4px solid rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-surface-variant));
}

.markdown-body :deep(ul),
.markdown-body :deep(ol) {
  margin-bottom: 16px;
  padding-left: 24px;
}

.markdown-body :deep(li) {
  margin-bottom: 4px;
}

.markdown-body :deep(hr) {
  border: none;
  border-top: 1px solid rgb(var(--v-theme-outline));
  margin: 24px 0;
}

@media (max-width: 600px) {
  .readme-wrapper {
    padding: 16px;
  }

  .markdown-body {
    font-size: 14px;
  }

  .markdown-body :deep(pre) {
    padding: 8px;
  }
}
</style>
