import { test } from "node:test";
import assert from "node:assert/strict";
import { cargoLoginCommand } from "./tokenSetup.ts";

test("cargo login names the Kellnr registry and leaves the token out of the command", () => {
  assert.equal(cargoLoginCommand(), "cargo login --registry kellnr");
});
