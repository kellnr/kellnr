import { test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import {
  assertDockerAvailable,
  createBufferedTestLogger,
  ensureLocalKellnrTestImage,
  publishCrate,
  restrictToSingleWorkerBecauseFixedPorts,
  waitForHttpOk,
} from "./testUtils";
import { startContainer, withStartedContainer } from "./lib/docker";

test.describe("auth required smoke test", () => {
  // This test relies on stable localhost:8000 URLs (cratesio proxy download URLs).
  restrictToSingleWorkerBecauseFixedPorts();

  test("starts kellnr with auth required and can publish crates", async ({}, testInfo) => {
    testInfo.setTimeout(10 * 60 * 1000);

    const tlog = createBufferedTestLogger(testInfo, "test-auth-req");
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

      // Use fixed localhost:8000 so Kellnr generates stable cratesio proxy download URLs.
      const hostPort = 8000;
      const baseUrl = `http://localhost:${hostPort}`;
      const url = baseUrl;

      const crateCargoConfigPath = path.resolve(
        process.cwd(),
        "crates",
        "test-auth-req",
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

      const containerBaseName = `kellnr-auth-req-${suffix}`;

      const started = await startContainer(
        {
          name: containerBaseName,
          image,
          ports: { 8000: hostPort },
          env: {
            KELLNR_LOG__LEVEL: "debug",
            KELLNR_LOG__LEVEL_WEB_SERVER: "debug",
            KELLNR_REGISTRY__AUTH_REQUIRED: "true",

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
              cratePath: "tests2/crates/test-auth-req/test_lib",
              registry,
              registryToken,
            });

            log("Publishing crate: foo-bar");
            await publishCrate({
              cratePath: "tests2/crates/test-auth-req/foo-bar",
              registry,
              registryToken,
            });

            log("Crate publishing finished");
          });
        },
        { alwaysCollectLogs: true },
      );

      log("Done");
    } finally {
      await tlog.flush();
    }
  });
});
