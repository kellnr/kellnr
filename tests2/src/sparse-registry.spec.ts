import { test } from "@playwright/test";
import path from "node:path";
import {
  assertDockerAvailable,
  createBufferedTestLogger,
  ensureLocalKellnrTestImage,
  publishCrate,
  waitForHttpOk,
  restrictToSingleWorkerBecauseFixedPorts,
} from "./testUtils";
import { startKellnr, withStartedKellnr } from "./lib/kellnr";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";

test.describe("sparse registry smoke test", () => {
  // Lua-style setup:
  // - Kellnr runs on fixed localhost:8000
  // - test crates keep their `.cargo/config.toml` pointing at localhost:8000
  // - crates.io proxy download URLs remain stable
  //
  // Because we bind a fixed host port, this suite must not run in parallel.
  restrictToSingleWorkerBecauseFixedPorts();

  test("starts kellnr with proxy enabled and can publish crates", async ({}, testInfo) => {
    testInfo.setTimeout(10 * 60 * 1000);

    const tlog = createBufferedTestLogger(testInfo, "sparse-registry");
    const log = tlog.log;

    // Keep names unique across parallel workers
    const suffix = `${testInfo.workerIndex}-${Date.now()}`;

    try {
      await test.step("check prerequisites", async () => {
        await assertDockerAvailable();
        log("Docker is available");
      });

      const image = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";
      const registry = "kellnr-test";

      const tokenSourceCrateDir = path.resolve(
        process.cwd(),
        "crates",
        "test-sparse-registry",
        "foo-bar",
      );

      const registryToken = extractRegistryTokenFromCargoConfig({
        crateDir: tokenSourceCrateDir,
        registryName: registry,
      });

      await test.step("ensure Kellnr test image exists (build if missing)", async () => {
        log(`Using image: ${image}`);
        await ensureLocalKellnrTestImage(image);
        log(`Image ready: ${image}`);
      });

      const started = await startKellnr(
        {
          name: `kellnr-sparse-registry-${suffix}`,
          image,
          env: {
            KELLNR_PROXY__ENABLED: "true",
          },
        },
        testInfo,
      );

      await withStartedKellnr(
        testInfo,
        started,
        async ({ baseUrl }) => {
          await test.step("wait for server readiness", async () => {
            log(`Waiting for HTTP 200 on ${baseUrl}`);
            await waitForHttpOk(baseUrl, {
              timeoutMs: 60_000,
              intervalMs: 1_000,
            });
            log("Server ready");
          });

          await test.step("publish crates", async () => {
            log("Publishing crate: test_lib");
            await publishCrate({
              cratePath: "tests2/crates/test-sparse-registry/test_lib",
              registry,
              registryToken,
            });

            log("Publishing crate: UpperCase-Name123");
            await publishCrate({
              cratePath: "tests2/crates/test-sparse-registry/UpperCase-Name123",
              registry,
              registryToken,
            });

            log("Publishing crate: foo-bar");
            await publishCrate({
              cratePath: "tests2/crates/test-sparse-registry/foo-bar",
              registry,
              registryToken,
            });
          });
        },
        // Prevent hang on teardown: only collect container logs on failure.
        { alwaysCollectLogs: false },
      );

      log("Done");
    } finally {
      await tlog.flush();
    }
  });
});
