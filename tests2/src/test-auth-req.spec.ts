import { test } from "@playwright/test";
import path from "node:path";
import {
  assertDockerAvailable,
  createBufferedTestLogger,
  ensureLocalKellnrTestImage,
  publishCrate,
  restrictToSingleWorkerBecauseFixedPorts,
  waitForHttpOk,
} from "./testUtils";
import { withStartedContainer } from "./lib/docker";
import { startKellnr, type StartedKellnr } from "./lib/kellnr";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";

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

      const tokenSourceCrateDir = path.resolve(
        process.cwd(),
        "crates",
        "test-auth-req",
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

      const startedKellnr: StartedKellnr = await startKellnr(
        {
          name: `kellnr-auth-req-${suffix}`,
          image,
          env: {
            KELLNR_REGISTRY__AUTH_REQUIRED: "true",
          },
        },
        testInfo,
      );

      await withStartedContainer(
        testInfo,
        startedKellnr.started,
        async () => {
          await test.step("wait for server readiness", async () => {
            log(`Waiting for HTTP 200 on ${startedKellnr.baseUrl}`);
            await waitForHttpOk(startedKellnr.baseUrl, {
              timeoutMs: 60_000,
              intervalMs: 1_000,
            });
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
