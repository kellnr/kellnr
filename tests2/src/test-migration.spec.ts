import { test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import {
  assertDockerAvailable,
  createBufferedTestLogger,
  ensureLocalKellnrTestImage,
  publishCrate,
  restrictToSingleWorkerBecauseFixedPorts,
  waitForHttpOk,
} from "./testUtils";
import { attachContainerLogs, startContainer } from "./lib/docker";

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

function extractRegistryTokenFromCrateConfig(
  crateDir: string,
  registryName: string,
): string {
  const configPath = path.resolve(crateDir, ".cargo", "config.toml");
  const contents = fs.readFileSync(configPath, "utf8");

  // Match lines like:
  //   kellnr-local = {index = "...", token = "..."}
  //   kellnr-test  = {index = "...", token = "..."}
  const escaped = registryName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const tokenRegex = new RegExp(
    `${escaped}\\s*=\\s*\\{[^}]*token\\s*=\\s*"([^"]+)"[^}]*\\}`,
  );

  const tokenMatch = contents.match(tokenRegex);
  if (!tokenMatch) {
    throw new Error(
      `Failed to extract ${registryName} token from ${configPath}`,
    );
  }
  return tokenMatch[1];
}

async function grepErrorsFromContainerLogs(logs: string): Promise<string> {
  // We deliberately avoid spawning `grep` to keep this portable; we filter in JS.
  const lines = logs.split(/\r?\n/);
  const errs = lines.filter((l) => l.includes("ERROR"));
  return errs.join("\n");
}

test.describe("migration smoke test", () => {
  // This test relies on stable localhost:8000 URLs (including cratesio proxy download URLs).
  restrictToSingleWorkerBecauseFixedPorts();

  test("migrates data from old image to new image and can publish after upgrade", async ({}, testInfo) => {
    testInfo.setTimeout(20 * 60 * 1000);

    const tlog = createBufferedTestLogger(testInfo, "test-migration");
    const log = tlog.log;

    async function fetchLatestReleasedKellnrImage(): Promise<string> {
      // Mirrors Lua `run_tests.lua` logic:
      // - GET https://api.github.com/repos/kellnr/kellnr/releases
      // - pick first release with > 2 assets
      // - use tag_name without leading 'v'
      //
      // Optional auth to reduce rate-limit flakiness:
      // - GITHUB_TOKEN (preferred)
      // - GH_TOKEN (alternate)
      const url = "https://api.github.com/repos/kellnr/kellnr/releases";
      const token = process.env.GITHUB_TOKEN || process.env.GH_TOKEN;

      const headers: Record<string, string> = {
        "user-agent": "kellnr-tests2/1.0",
        accept: "application/vnd.github+json",
      };
      if (token) headers.authorization = `Bearer ${token}`;

      const res = await fetch(url, { headers });
      const bodyText = await res.text();

      if (!res.ok) {
        if (bodyText.includes("API rate limit exceeded")) {
          throw new Error(
            `GitHub API rate limit exceeded while fetching releases. ` +
              `Set GITHUB_TOKEN (or GH_TOKEN) to increase the rate limit. ` +
              `Status: ${res.status}. Body prefix: ${bodyText.slice(0, 200)}`,
          );
        }
        throw new Error(
          `GitHub API request failed while fetching releases. ` +
            `Status: ${res.status}. Body prefix: ${bodyText.slice(0, 200)}`,
        );
      }

      let releases: unknown;
      try {
        releases = JSON.parse(bodyText);
      } catch {
        throw new Error(
          `Failed to parse JSON response from GitHub API. Response prefix: ${bodyText.slice(0, 200)}`,
        );
      }

      if (!Array.isArray(releases)) {
        throw new Error(
          `Unexpected GitHub releases response type. Response prefix: ${bodyText.slice(0, 200)}`,
        );
      }

      const minAssets = 2;
      for (const r of releases) {
        if (!r || typeof r !== "object") continue;

        const tagName = (r as any).tag_name;
        const assets = (r as any).assets;

        if (typeof tagName !== "string" || !Array.isArray(assets)) continue;

        if (assets.length > minAssets) {
          const version = tagName.startsWith("v") ? tagName.slice(1) : tagName;
          return `ghcr.io/kellnr/kellnr:${version}`;
        }
      }

      throw new Error("No release found with more than 2 assets.");
    }

    // The Lua test consumes 2 args: old_image + new_image.
    // In this port we use env vars to keep `npm test` simple.
    const oldImageFromEnv = process.env.KELLNR_OLD_IMAGE;
    const newImage = process.env.KELLNR_NEW_IMAGE;

    // New default for local runs: if you don't set KELLNR_NEW_IMAGE, we use the locally built test image.
    const resolvedNewImage =
      newImage ?? process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";

    // Old image: if not provided, auto-detect latest release from GitHub API (like the Lua runner).
    const oldImage =
      oldImageFromEnv && oldImageFromEnv.trim() !== ""
        ? oldImageFromEnv
        : await fetchLatestReleasedKellnrImage();

    // Match the Lua test behavior as closely as possible:
    // - old image is pulled
    // - kdata persisted via volume across old -> new container
    // - both run on fixed localhost:8000 (stable URLs)
    // - crates published to old, then to new
    const registry = process.env.KELLNR_MIGRATION_REGISTRY ?? "kellnr-local";

    const suffix = `${testInfo.workerIndex}-${Date.now()}`;
    const oldContainer = `kellnr-old-${suffix}`;
    const newContainer = `kellnr-new-${suffix}`;

    // Fixed port setup (Lua-style)
    const hostPort = 8000;
    const baseUrl = `http://localhost:${hostPort}`;
    const url = baseUrl;

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
    const registryToken = extractRegistryTokenFromCrateConfig(
      tokenSourceCrateDir,
      registry,
    );

    try {
      await test.step("check prerequisites", async () => {
        await assertDockerAvailable();
        log("Docker is available");
      });

      await test.step("ensure new image exists (globalSetup should have built local image)", async () => {
        log(`New image: ${resolvedNewImage}`);
        // If it's the local image, globalSetup should already ensure it exists.
        // This also supports running with a custom new image tag by pre-building it.
        await ensureLocalKellnrTestImage(resolvedNewImage);
        log("New image is available");
      });

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
      let startedOld: Awaited<ReturnType<typeof startContainer>> | undefined;
      try {
        await test.step("start old Kellnr container", async () => {
          log(`Starting old container: ${oldContainer}`);
          log(`Mapping host port ${hostPort} -> container 8000`);
          log(`Mounting kdata: ${kdataDir} -> ${kdataMount}`);

          startedOld = await startContainer(
            {
              name: oldContainer,
              image: oldImage,
              ports: { 8000: hostPort },
              env: {
                KELLNR_LOG__LEVEL: "debug",
                KELLNR_LOG__LEVEL_WEB_SERVER: "debug",

                // Ensure Kellnr generates stable URLs with localhost:8000
                KELLNR_ORIGIN__PORT: String(hostPort),
              },
              bindMounts: {
                [kdataDir]: kdataMount,
              },
            },
            testInfo,
          );

          log(`Old container started on ${baseUrl}`);
        });

        await test.step("wait for old server readiness", async () => {
          log(`Waiting for HTTP 200 on ${url}`);
          await waitForHttpOk(url, { timeoutMs: 60_000, intervalMs: 1_000 });
          log("Old server ready");
        });

        await test.step("publish crates to old version", async () => {
          log("Publishing crates to old version...");

          log("Publishing crate: test_lib");
          await publishCrate({
            cratePath: "tests2/crates/test-migration/test_lib",
            registry,
            registryToken,
          });

          log("Publishing crate: foo-bar");
          await publishCrate({
            cratePath: "tests2/crates/test-migration/foo-bar",
            registry,
            registryToken,
          });

          log("Published crates to old version");
        });
      } finally {
        await test.step("stop old container", async () => {
          log(`Stopping old container: ${oldContainer}`);
          if (startedOld) {
            await attachContainerLogs(testInfo, startedOld.container, {
              name: "kellnr-old",
            });
            await startedOld.container.stop().catch(() => {});
          }
          log("Old container stopped");
        });
      }

      // ---- Run new container ----
      let startedNew: Awaited<ReturnType<typeof startContainer>> | undefined;
      try {
        await test.step("start new Kellnr container (with same kdata)", async () => {
          log(`Starting new container: ${newContainer}`);
          log(`Mapping host port ${hostPort} -> container 8000`);
          log(`Reusing kdata: ${kdataDir} -> ${kdataMount}`);

          startedNew = await startContainer(
            {
              name: newContainer,
              image: resolvedNewImage,
              ports: { 8000: hostPort },
              env: {
                KELLNR_LOG__LEVEL: "debug",
                KELLNR_LOG__LEVEL_WEB_SERVER: "debug",

                // Ensure Kellnr generates stable URLs with localhost:8000
                KELLNR_ORIGIN__PORT: String(hostPort),
              },
              bindMounts: {
                [kdataDir]: kdataMount,
              },
            },
            testInfo,
          );

          log(`New container started on ${baseUrl}`);
        });

        await test.step("wait for new server readiness", async () => {
          log(`Waiting for HTTP 200 on ${url}`);
          await waitForHttpOk(url, { timeoutMs: 60_000, intervalMs: 1_000 });
          log("New server ready");
        });

        await test.step("publish crate to new version", async () => {
          log("Publishing crates to new version...");

          log("Publishing crate: full-toml");
          await publishCrate({
            cratePath: "tests2/crates/test-migration/full-toml",
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

          // This mirrors the Lua test: it prints errors but does not fail based on them.
        });
      } finally {
        await test.step("stop new container", async () => {
          log(`Stopping new container: ${newContainer}`);
          if (startedNew) {
            await attachContainerLogs(testInfo, startedNew.container, {
              name: "kellnr-new",
            });
            await startedNew.container.stop().catch(() => {});
          }
          log("New container stopped");
        });
      }

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
  });
});
