import { test } from "node:test";
import assert from "node:assert/strict";
import { errorDetail } from "./apiError.ts";

test("reads the detail without the cargo prefix", () => {
  assert.equal(errorDetail({ errors: [{ detail: "ERROR: Group name is empty" }] }), "Group name is empty");
});

test("keeps a detail without prefix", () => {
  assert.equal(errorDetail({ errors: [{ detail: "Not found" }] }), "Not found");
});

test("ignores bodies without a detail", () => {
  assert.equal(errorDetail(undefined), undefined);
  assert.equal(errorDetail(null), undefined);
  assert.equal(errorDetail("plain text"), undefined);
  assert.equal(errorDetail({}), undefined);
  assert.equal(errorDetail({ errors: [] }), undefined);
  assert.equal(errorDetail({ errors: [{ detail: 42 }] }), undefined);
  assert.equal(errorDetail({ errors: [{ detail: "ERROR: " }] }), undefined);
});
