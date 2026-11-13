import { test } from "node:test";
import assert from "node:assert/strict";
import { demoteHeadingDepth, renderReadme } from "./readme.ts";

test("demotes headings by one level, h6 stays h6", () => {
  assert.deepEqual([1, 2, 3, 4, 5, 6].map(demoteHeadingDepth), [2, 3, 4, 5, 6, 6]);
});

test("renders readme headings one level lower with their anchors", () => {
  const html = renderReadme("# Title\n\n## Usage\n\n###### Deep");
  assert.match(html, /<h2 id="title">Title<\/h2>/);
  assert.match(html, /<h3 id="usage">Usage<\/h3>/);
  assert.match(html, /<h6 id="deep">Deep<\/h6>/);
  assert.doesNotMatch(html, /<h1/);
});

test("highlights code blocks", () => {
  const html = renderReadme("```rust\nfn main() {}\n```");
  assert.match(html, /class="hljs language-rust"/);
});
