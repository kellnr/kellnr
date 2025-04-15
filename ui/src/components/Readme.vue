<template>
  <v-card class="mx-auto readme-container" elevation="1" rounded="lg">
    <v-card-text>
      <div class="markdown-body" v-html="markedReadme"></div>
    </v-card-text>
  </v-card>
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

<style>
.markdown-body {
  font-family: inherit;
}

.markdown-body pre {
  border-radius: 4px;
  margin: 16px 0;
  padding: 16px;
  overflow-x: auto;
}

.markdown-body code:not(pre code) {
  color: inherit;
  padding: 0.2em 0.4em;
  border-radius: 3px;
  font-size: 0.9em;
  background-color: rgba(175, 184, 193, 0.2);
}

.markdown-body h1,
.markdown-body h2,
.markdown-body h3,
.markdown-body h4,
.markdown-body h5,
.markdown-body h6 {
  margin-top: 24px;
  margin-bottom: 16px;
  font-weight: 600;
  line-height: 1.25;
}

.markdown-body h1 {
  font-size: 2em;
}

.markdown-body h2 {
  font-size: 1.5em;
}

.markdown-body h3 {
  font-size: 1.25em;
}

.markdown-body p {
  margin-bottom: 16px;
}

.markdown-body img {
  max-width: 100%;
  height: auto;
}

.markdown-body table {
  border-collapse: collapse;
  width: 100%;
  margin-bottom: 16px;
}

.markdown-body table th,
.markdown-body table td {
  border: 1px solid;
  padding: 6px 13px;
}

/* Make the readme container responsive */
.readme-container {
  width: 100%;
  max-width: 960px;
  margin: 0 auto;
}

@media (max-width: 600px) {
  .markdown-body {
    font-size: 14px;
  }

  .markdown-body pre {
    padding: 8px;
  }
}
</style>
