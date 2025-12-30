import { test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import {
  assertDockerAvailable,
  createBufferedTestLogger,
  ensureLocalKellnrTestImage,
  publishCrate,
  waitForHttpOk,
  restrictToSingleWorkerBecauseFixedPorts,
} from "./testUtils";
import { startContainer, withStartedContainer } from "./lib/docker";

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
      const containerBaseName = `kellnr-sparse-registry-${suffix}`;

      const image = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";
      const registry = "kellnr-test";

      // Fixed localhost:8000
      const hostPort = 8000;
      const baseUrl = `http://localhost:${hostPort}`;
      const url = baseUrl;

      const crateCargoConfigPath = path.resolve(
        process.cwd(),
        "crates",
        "test-sparse-registry",
        "foo-bar",
        ".cargo",
        "config.toml",
      );
      const crateCargoConfig = fs.readFileSync(crateCargoConfigPath, "utf8");
      const tokenMatch = crateCargoConfig.match(
        /kellnr-test\s*=\s*\{[^}]*token\s*=\s*"([^"]+)"[^}]*\}/,
      );
      if (!tokenMatch) {
        throw new Error(
          `Failed to extract kellnr-test token from ${crateCargoConfigPath}`,
        );
      }
      const registryToken = tokenMatch[1];

      await test.step("ensure Kellnr test image exists (build if missing)", async () => {
        log(`Using image: ${image}`);
        await ensureLocalKellnrTestImage(image);
        log(`Image ready: ${image}`);
      });

      const started = await startContainer(
        {
          name: containerBaseName,
          image,
          ports: { 8000: hostPort },
          env: {
            KELLNR_LOG__LEVEL: "debug",
            KELLNR_LOG__LEVEL_WEB_SERVER: "debug",
            KELLNR_PROXY__ENABLED: "true",

            // Ensure Kellnr generates URLs with localhost:8000 (cratesio proxy download URLs)
            KELLNR_ORIGIN__PORT: String(hostPort),
          },
        },
        testInfo,
      );

      await withStartedContainer(
        testInfo,
        started,
        async () => {
          await test.step("wait for server readiness", async () => {
            log(`Waiting for HTTP 200 on ${url}`);
            await waitForHttpOk(url, { timeoutMs: 60_000, intervalMs: 1_000 });
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
