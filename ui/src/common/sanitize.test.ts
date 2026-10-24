import { test } from "node:test";
import assert from "node:assert/strict";
import { README_SANITIZE_CONFIG } from "./sanitize.ts";

test("README sanitizer forbids styling and form elements", () => {
  for (const tag of ["style", "form", "input", "button", "textarea", "select"]) {
    assert.ok(README_SANITIZE_CONFIG.FORBID_TAGS.includes(tag), tag);
  }
  assert.deepEqual(README_SANITIZE_CONFIG.FORBID_ATTR, ["style"]);
});
