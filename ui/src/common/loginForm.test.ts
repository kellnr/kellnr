import { test } from "node:test";
import assert from "node:assert/strict";
import { loginValidateOn, required } from "./loginForm.ts";

test("required rule passes for a value", () => {
  assert.equal(required("Username")("admin"), true);
});

test("required rule names the empty field", () => {
  assert.equal(required("Username")(""), "Username is required");
  assert.equal(required("Password")(null), "Password is required");
});

test("fields are validated on submit until a submit failed, then on input", () => {
  assert.equal(loginValidateOn(false), "submit");
  assert.equal(loginValidateOn(true), "input");
});
