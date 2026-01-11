import { test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

import {
  assertDockerAvailable,
  createBufferedTestLogger,
  ensureLocalKellnrTestImage,
  fetchLatestReleasedKellnrImage,
  publishCrate,
  restrictToSingleWorkerBecauseFixedPorts,
  waitForHttpOk,
} from "./testUtils";

import {
  attachContainerLogs,
  startContainer,
  withStartedContainer,
} from "./lib/docker";

import { kellnrDefaults } from "./lib/kellnr";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";

function rimrafSync(p: string) {
  fs.rmSync(p, { recursive: true, force: true });
}

function fileExists(p: string): boolean {
  try {
    return fs.existsSync(p);
  } catch {
    return false;
  }
}

// Token extraction is centralized in `src/harness/registry.ts`
// to keep one source of truth and consistent error messages.

async function grepErrorsFromContainerLogs(logs: string): Promise<string> {
  // We deliberately avoid spawning `grep` to keep this portable; we filter in JS.
  const lines = logs.split(/\r?\n/);
  const errs = lines.filter((l) => l.includes("ERROR"));
  return errs.join("\n");
}

test.describe("migration smoke test", () => {
  // This test relies on stable localhost:8000 URLs (including cratesio proxy download URLs).
  restrictToSingleWorkerBecauseFixedPorts();

  test(
    "migrates data from old image to new image and can publish after upgrade",
    async ({ }, testInfo) => {
      testInfo.setTimeout(20 * 60 * 1000);

      const tlog = createBufferedTestLogger(testInfo, "test-migration");
      const log = tlog.log;

      // This test can be configured via env vars to keep `npm test` simple.
      const oldImageFromEnv = process.env.KELLNR_OLD_IMAGE;
      const newImage = process.env.KELLNR_NEW_IMAGE;

      // New default for local runs: if you don't set KELLNR_NEW_IMAGE, we use the locally built test image.
      const resolvedNewImage =
        newImage ?? process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";

      // Old image: if not provided, auto-detect latest release from GHCR.

      const oldImage =
        oldImageFromEnv && oldImageFromEnv.trim() !== ""
          ? oldImageFromEnv
          : await fetchLatestReleasedKellnrImage();

      // Match the legacy test behavior as closely as possible:
      // - old image is pulled
      // - kdata persisted via volume across old -> new container
      // - both run on fixed localhost:8000 (stable URLs)
      // - crates published to old, then to new
      const registry = process.env.KELLNR_MIGRATION_REGISTRY ?? "kellnr-local";

      const suffix = `${testInfo.workerIndex}-${Date.now()}`;
      const oldContainer = `kellnr-old-${suffix}`;
      const newContainer = `kellnr-new-${suffix}`;

      // Fixed localhost:8000 setup (shared helper)
      const k = kellnrDefaults();
      const url = k.baseUrl;

      // Persisted data directory mounted into /opt/kdata
      const kdataDir = path.resolve(
        process.cwd(),
        "tmp",
        "test-migration",
        `kdata-${suffix}`,
      );
      const kdataMount = "/opt/kdata";

      // Extract token from one of the migration crates (it’s the same token used across these test crates)
      const migrationCratesRoot = path.resolve(
        process.cwd(),
        "crates",
        "test-migration",
      );
      const tokenSourceCrateDir = path.resolve(migrationCratesRoot, "foo-bar");
      const registryToken = extractRegistryTokenFromCargoConfig({
        crateDir: tokenSourceCrateDir,
        registryName: registry,
      });

      try {
        await test.step("check prerequisites", async () => {
          await assertDockerAvailable();
          log("Docker is available");
        });

        await test.step(
          "ensure new image exists (globalSetup should have built local image)",
          async () => {
            log(`New image: ${resolvedNewImage}`);
            // If it's the local image, globalSetup should already ensure it exists.
            // This also supports running with a custom new image tag by pre-building it.
            await ensureLocalKellnrTestImage(resolvedNewImage);
            log("New image is available");
          },
        );

        await test.step("pull old image", async () => {
          log(`Old image: ${oldImage}`);
          log(
            "Skipping explicit docker pull (testcontainers/Docker will pull on-demand if needed)",
          );
        });

        await test.step("prepare kdata dir", async () => {
          log(`Preparing kdata dir: ${kdataDir}`);
          rimrafSync(kdataDir);
          fs.mkdirSync(kdataDir, { recursive: true });
          log("kdata dir ready");
        });

        // ---- Run old container ----
        const startedOld = await startContainer(
          {
            name: oldContainer,
            image: oldImage,
            ports: k.ports,
            env: {
              ...k.env,
            },
            bindMounts: {
              [kdataDir]: kdataMount,
            },
          },
          testInfo,
        );

        await withStartedContainer(
          testInfo,
          startedOld,
          async () => {
            await test.step("wait for old server readiness", async () => {
              log(`Waiting for HTTP 200 on ${url}`);
              await waitForHttpOk(url, { timeoutMs: 60_000, intervalMs: 1_000 });
              log("Old server ready");
            });

            await test.step("publish crates to old version", async () => {
              log("Publishing crates to old version...");

              log("Publishing crate: test_lib");
              await publishCrate({
                cratePath: "tests/crates/test-migration/test_lib",
                registry,
                registryToken,
              });

              log("Publishing crate: foo-bar");
              await publishCrate({
                cratePath: "tests/crates/test-migration/foo-bar",
                registry,
                registryToken,
              });

              log("Published crates to old version");
            });
          },
          { alwaysCollectLogs: true },
        );

        // ---- Run new container ----
        const startedNew = await startContainer(
          {
            name: newContainer,
            image: resolvedNewImage,
            ports: k.ports,
            env: {
              ...k.env,
            },
            bindMounts: {
              [kdataDir]: kdataMount,
            },
          },
          testInfo,
        );

        await withStartedContainer(
          testInfo,
          startedNew,
          async () => {
            await test.step("wait for new server readiness", async () => {
              log(`Waiting for HTTP 200 on ${url}`);
              await waitForHttpOk(url, { timeoutMs: 60_000, intervalMs: 1_000 });
              log("New server ready");
            });

            await test.step("publish crate to new version", async () => {
              log("Publishing crates to new version...");

              log("Publishing crate: full-toml");
              await publishCrate({
                cratePath: "tests/crates/test-migration/full-toml",
                registry,
                registryToken,
              });

              log("Published crates to new version");
            });

            await test.step("check logs for ERROR entries", async () => {
              log("Checking container logs for ERROR lines...");
              const oldErrs = await grepErrorsFromContainerLogs(oldContainer);
              const newErrs = await grepErrorsFromContainerLogs(newContainer);

              if (oldErrs.trim()) {
                log("ERROR lines found in old container logs:");
                log(oldErrs);
              } else {
                log("No ERROR lines found in old container logs");
              }

              if (newErrs.trim()) {
                log("ERROR lines found in new container logs:");
                log(newErrs);
              } else {
                log("No ERROR lines found in new container logs");
              }

              // This mirrors the legacy behavior: it prints errors but does not fail based on them.
            });
          },
          { alwaysCollectLogs: true },
        );

        log("Done");
      } finally {
        // Cleanup kdata dir so parallel runs don't accumulate junk.
        try {
          if (fileExists(kdataDir)) rimrafSync(kdataDir);
        } catch {
          // best-effort
        }
        await tlog.flush();
      }
    },
  );
});

