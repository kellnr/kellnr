/**
 * End-to-end tests for the license filter with cargo.
 *
 * Kellnr runs with the license filter enabled for the crates.io proxy and the
 * registry, allowing only MIT. Cargo fetches the dependencies of test crates
 * through Kellnr:
 * - Allowed licenses are downloaded
 * - A Kellnr crate without a license is denied, cargo shows the reason
 * - A crates.io crate with a license that is not allowed is denied, cargo
 *   shows the reason
 *
 * Every fetch uses an empty CARGO_HOME, so no crate comes from a cache of an
 * earlier run.
 */

import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { test, expect } from "./lib/ui-fixtures";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  publishCrate,
  assertKellnrBinaryExists,
  exec,
  repoRootFromTests,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";

const CRATES_DIR = "tests/crates/test-license-filter";

/** Run `cargo fetch` for a test crate with an empty CARGO_HOME. */
async function cargoFetch(crate: string) {
  const crateDir = path.resolve(repoRootFromTests(), CRATES_DIR, crate);
  const lockFile = path.join(crateDir, "Cargo.lock");
  fs.rmSync(lockFile, { force: true });
  const cargoHome = fs.mkdtempSync(path.join(os.tmpdir(), "kellnr-license-filter-"));
  try {
    return await exec("cargo", ["fetch"], {
      cwd: crateDir,
      env: { ...process.env, CARGO_HOME: cargoHome },
      timeout: 120_000,
    });
  } finally {
    fs.rmSync(cargoHome, { recursive: true, force: true });
    fs.rmSync(lockFile, { force: true });
  }
}

test.describe("License filter with cargo", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;

  test.beforeAll(async () => {
    test.setTimeout(120_000);

    assertKellnrBinaryExists();

    started = await startLocalKellnr({
      name: `kellnr-license-filter-${Date.now()}`,
      env: {
        KELLNR_PROXY__ENABLED: "true",
        KELLNR_CRATESIO_FILTER__LICENSE_ENABLED: "true",
        KELLNR_CRATESIO_FILTER__LICENSE_ALLOWED: "MIT",
        KELLNR_REGISTRY_FILTER__LICENSE_ENABLED: "true",
        KELLNR_REGISTRY_FILTER__LICENSE_ALLOWED: "MIT",
      },
    });
    console.log(`[setup] Server ready at ${started.baseUrl}`);

    const registry = "kellnr-test";
    const registryToken = extractRegistryTokenFromCargoConfig({
      crateDir: path.resolve(repoRootFromTests(), CRATES_DIR, "mit-lib"),
      registryName: registry,
    });
    // Publishing is not filtered, only downloads are
    for (const crate of ["mit-lib", "unlicensed-lib"]) {
      console.log(`[setup] Publishing crate: ${crate}`);
      await publishCrate({
        cratePath: `${CRATES_DIR}/${crate}`,
        registry,
        registryToken,
      });
    }
  });

  test.afterAll(async () => {
    if (started) {
      await started.stop();
    }
  });

  test("cargo downloads crates with allowed licenses", async () => {
    const result = await cargoFetch("consumer-allowed");

    expect(result.exitCode, result.stderr).toBe(0);
  });

  test("cargo shows why a Kellnr crate without license is denied", async () => {
    const result = await cargoFetch("consumer-denied-registry");

    expect(result.exitCode).not.toBe(0);
    expect(result.stderr).toContain("403");
    expect(result.stderr).toContain(
      "Download of unlicensed-lib@1.0.0 denied by Kellnr license filter: the license is unknown"
    );
  });

  test("cargo shows why a crates.io crate with another license is denied", async () => {
    const result = await cargoFetch("consumer-denied-cratesio");

    expect(result.exitCode).not.toBe(0);
    expect(result.stderr).toContain("403");
    expect(result.stderr).toContain(
      'Download of ryu@1.0.23 denied by Kellnr license filter: license "Apache-2.0 OR BSL-1.0" is not allowed'
    );
  });
});
