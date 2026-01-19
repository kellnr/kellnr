<template>
  <div class="readme-wrapper">
    <div class="markdown-body" v-html="markedReadme"></div>
  </div>
</template>

<script setup lang="ts">
import { Marked } from 'marked';
import { mangle } from 'marked-mangle';
import { gfmHeadingId } from "marked-gfm-heading-id";
import { markedHighlight } from "marked-highlight";
import DOMPurify from 'dompurify';
import { computed, onMounted, watchEffect } from "vue";
import hljs from 'highlight.js';
import 'highlight.js/styles/github.css';
import { useTheme } from 'vuetify';

const theme = useTheme();

const marked = new Marked(
  markedHighlight({
    langPrefix: 'hljs language-',
    highlight(code, lang) {
      const language = hljs.getLanguage(lang) ? lang : 'plaintext';
      return hljs.highlight(code, { language }).value;
    }
  })
);
marked.use(mangle());
marked.use(gfmHeadingId());

const props = defineProps<{
  readme: string | null
}>();

const markedReadme = computed(() => {
  return DOMPurify.sanitize(marked.parse(props.readme || ''));
});

// Switch highlight.js theme based on current Vuetify theme
watchEffect(() => {
  const linkId = 'highlight-theme';
  let link = document.getElementById(linkId) as HTMLLinkElement;

  if (!link) {
    link = document.createElement('link');
    link.id = linkId;
    link.rel = 'stylesheet';
    document.head.appendChild(link);
  }

  const themeName = theme.global.current.value.dark ? 'github-dark' : 'github';
  link.href = `https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.7.0/build/styles/${themeName}.min.css`;
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
  border-radius: 8px;
  margin: 16px 0;
  padding: 16px;
  overflow-x: auto;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
}

.markdown-body :deep(code:not(pre code)) {
  color: inherit;
  padding: 0.2em 0.4em;
  border-radius: 4px;
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

.markdown-body :deep(h1) {
  font-size: 2em;
}

.markdown-body :deep(h2) {
  font-size: 1.5em;
}

.markdown-body :deep(h3) {
  font-size: 1.25em;
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
