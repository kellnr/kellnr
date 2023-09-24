<template>

  <div id="mdContainer" class="glass" v-html="markedReadme">
  </div>

</template>

<script setup lang="ts">
import {Marked} from 'marked';
import {mangle} from 'marked-mangle';
import { gfmHeadingId } from "marked-gfm-heading-id";
import {markedHighlight} from "marked-highlight";
import DOMPurify from 'dompurify';
import {computed} from "vue";
import hljs from 'highlight.js';

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
}>()

const markedReadme = computed(() => {
  // Use marked.js with heighlight.js to render the readme
  return DOMPurify.sanitize(marked.parse(props.readme || ''))
})

</script>

<style>
#mdContainer > pre {
  /*
  Line break for code instead of re-size of the code block to fit all text
  which breaks the layout.
  Better would be a scroll bar, but that seems not possible
  */
  white-space: pre-wrap;
}

#mdContainer > h1 {
  font-size: 2rem !important;
  font-weight: 700;
  margin-bottom: 1rem;
}

#mdContainer > h2 {
  font-size: 1.5rem;
  font-weight: 700;
  margin-bottom: 1rem;
}

#mdContainer > h3 {
  font-size: 1.2rem;
  font-weight: 700;
  margin-bottom: 1rem;
}

#mdContainer > h4 {
  font-size: 1.1rem;
  font-weight: 700;
  margin-bottom: 1rem;
}

#mdContainer > h5 {
  font-size: 1.1rem;
  font-weight: 700;
  margin-bottom: 1rem;
}

#mdContainer > h6 {
  font-size: 1.1rem;
  font-weight: 700;
  margin-bottom: 1rem;
}

#mdContainer > p {
  margin-bottom: 0.7rem;
  margin-top: 0.5rem;
}

#mdContainer > * code {
  background-color: transparent;
  padding: 0;
  font-size: 1rem;
}

/*Dark theme*/
body[color-theme="dark"] #mdContainer > * strong {
  color: var(--dark-color-white);
}

body[color-theme="dark"] #mdContainer > * code {
  background-color: transparent;
  padding: 0;
  font-size: 1rem;
  color: var(--dark-color-white);
}


body[color-theme="dark"] #mdContainer > h1 {
  /*Gradient text*/
  background: linear-gradient(to right, var(--dark-color-middle) 0%, var(--dark-color-dark) 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;

}

body[color-theme="dark"] #mdContainer > h2 {
  /*Gradient text*/
  background: linear-gradient(to right, var(--dark-color-middle) 0%, var(--dark-color-dark) 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;

}

body[color-theme="dark"] #mdContainer > h3 {
  /*Gradient text*/
  background: linear-gradient(to right, var(--dark-color-middle) 0%, var(--dark-color-dark) 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;

}

body[color-theme="dark"] #mdContainer > h4 {
  /*Gradient text*/
  background: linear-gradient(to right, var(--dark-color-middle) 0%, var(--dark-color-dark) 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;

}

body[color-theme="dark"] #mdContainer > h5 {
  /*Gradient text*/
  background: linear-gradient(to right, var(--dark-color-middle) 0%, var(--dark-color-dark) 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;

}

body[color-theme="dark"] #mdContainer > h6 {
  /*Gradient text*/
  background: linear-gradient(to right, var(--dark-color-middle) 0%, var(--dark-color-dark) 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;

}
</style>
