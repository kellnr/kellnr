import { test } from "node:test";
import assert from "node:assert/strict";
import { cargoAddCommand, cargoTomlDependency } from "./registry.ts";

test("Cargo.toml dependency names the Kellnr registry", () => {
  assert.equal(
    cargoTomlDependency("foo-bar", "1.0.0"),
    'foo-bar = { version = "1.0.0", registry = "kellnr" }',
  );
});

test("cargo add command names the Kellnr registry", () => {
  assert.equal(cargoAddCommand("foo-bar"), "cargo add foo-bar --registry kellnr");
});
