import { Marked } from "marked";
import { mangle } from "marked-mangle";
import { gfmHeadingId } from "marked-gfm-heading-id";
import { markedHighlight } from "marked-highlight";
import hljs from "highlight.js";

/** The crate name is the page's h1, so README headings start at h2 */
export function demoteHeadingDepth(depth: number): number {
  return Math.min(depth + 1, 6);
}

const marked = new Marked(
  markedHighlight({
    langPrefix: "hljs language-",
    highlight(code, lang) {
      const language = hljs.getLanguage(lang) ? lang : "plaintext";
      return hljs.highlight(code, { language }).value;
    },
  }),
);
marked.use(mangle());
marked.use(gfmHeadingId());
marked.use({
  walkTokens(token) {
    if (token.type === "heading") {
      token.depth = demoteHeadingDepth(token.depth);
    }
  },
});

/** README markdown as HTML, not sanitized */
export function renderReadme(markdown: string): string {
  return marked.parse(markdown, { async: false });
}
