/**
 * UI tests for database migration and version upgrades.
 *
 * Tests:
 * - Starts Kellnr with old version (Docker) and publishes crates
 * - Stops old container
 * - Starts Kellnr with new version (local) using the same data directory
 * - Verifies database migration succeeded
 * - Verifies UI is accessible and shows migrated data
 *
 * Performance: Sequential test with old (Docker) → new (local) version upgrade.
 * Docker is used for old version because we need released images.
 * Local process is used for new version to speed up testing.
 */

import { test, expect } from "./lib/ui-fixtures";
import { LandingPage, CratesPage, CratePage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  waitForHttpOk,
  assertDockerAvailable,
  assertKellnrBinaryExists,
  publishCrate,
  fetchLatestReleasedKellnrImage,
} from "./testUtils";
import { startContainer, attachContainerLogs } from "./lib/docker";
import { kellnrDefaults } from "./lib/kellnr";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { execa } from "execa";

function rimrafSync(p: string) {
  fs.rmSync(p, { recursive: true, force: true });
}

/**
 * Fix ownership of files in a directory using Docker.
 * Uses a lightweight container to chown files - no sudo required.
 *
 * On macOS with Docker Desktop, files are typically already owned by the current user,
 * so no fix is needed.
 */
async function fixDataDirOwnershipWithDocker(dirPath: string): Promise<void> {
  const uid = typeof process.getuid === "function" ? process.getuid() : null;
  const gid = typeof process.getgid === "function" ? process.getgid() : null;

  if (uid === null || gid === null) {
    console.log("[migration] Cannot determine current user UID/GID, skipping ownership fix");
    return;
  }

  // Check if files need ownership fix
  try {
    const entries = fs.readdirSync(dirPath);
    if (entries.length === 0) {
      console.log("[migration] Data directory is empty, no fix needed");
      return;
    }

    const firstEntry = path.join(dirPath, entries[0]);
    const stat = fs.statSync(firstEntry);
    if (stat.uid === uid) {
      console.log("[migration] Data directory contents already owned by current user, no fix needed");
      return;
    }
    console.log(`[migration] Data directory contents owned by uid ${stat.uid}, current user is ${uid}`);
  } catch (e) {
    console.log(`[migration] Could not check data directory contents: ${e}`);
  }

  // Use Docker to fix ownership - no sudo required
  try {
    console.log(`[migration] Fixing ownership with Docker to ${uid}:${gid}`);
    await execa("docker", [
      "run", "--rm",
      "-v", `${dirPath}:/data`,
      "busybox",
      "chown", "-R", `${uid}:${gid}`, "/data"
    ], { stdio: "inherit" });
    console.log("[migration] Ownership fixed via Docker");
  } catch (e) {
    console.log(`[migration] Warning: Could not fix ownership with Docker: ${e}`);
  }
}

function fileExists(p: string): boolean {
  try {
    return fs.existsSync(p);
  } catch {
    return false;
  }
}

test.describe("Migration UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let kdataDir: string;
  let baseUrl: string;

  test("migrates from old version to new and UI is accessible", async ({ page }, testInfo) => {
    testInfo.setTimeout(20 * 60 * 1000); // 20 minutes

    // Get old image (must use Docker for released versions)
    const oldImageFromEnv = process.env.KELLNR_OLD_IMAGE;
    const oldImage =
      oldImageFromEnv && oldImageFromEnv.trim() !== ""
        ? oldImageFromEnv
        : await fetchLatestReleasedKellnrImage();

    console.log(`[migration] Old image (Docker): ${oldImage}`);
    console.log("[migration] New version: local binary");

    const registry = process.env.KELLNR_MIGRATION_REGISTRY ?? "kellnr-local";
    const suffix = `${Date.now()}`;
    const oldContainer = `kellnr-old-${suffix}`;

    // Fixed localhost:8000 setup
    const k = kellnrDefaults();
    baseUrl = k.baseUrl;

    // Persisted data directory - used by both Docker and local Kellnr
    kdataDir = path.resolve(
      process.cwd(),
      "tmp",
      "ui-test-migration",
      `kdata-${suffix}`,
    );
    const kdataMount = "/opt/kdata";

    // Extract registry token
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

    let localKellnr: StartedLocalKellnr | undefined;

    try {
      await test.step("check prerequisites", async () => {
        await assertDockerAvailable();
        console.log("[migration] Docker is available (for old version)");

        assertKellnrBinaryExists();
        console.log("[migration] Local Kellnr binary is available (for new version)");
      });

      await test.step("prepare kdata dir", async () => {
        console.log(`[migration] Preparing kdata dir: ${kdataDir}`);
        rimrafSync(kdataDir);
        fs.mkdirSync(kdataDir, { recursive: true });
        console.log("[migration] kdata dir ready");
      });

      // ---- Run old container (Docker) ----
      await test.step("run old version (Docker) and publish crates", async () => {
        const startedOld = await startContainer(
          {
            name: oldContainer,
            image: oldImage,
            ports: k.ports,
            env: {
              ...k.env,
              // Explicitly set data directory to ensure it uses the mounted path
              KELLNR_REGISTRY__DATA_DIR: kdataMount,
              // Since v6.0.0 the Docker image no longer provisions a default admin
              // token.  We need to set it explicitly so `cargo publish` can authenticate.
              KELLNR_SETUP__ADMIN_TOKEN: registryToken,
            },
            bindMounts: {
              [kdataDir]: kdataMount,
            },
          },
          testInfo,
        );

        try {
          console.log(`[migration] Waiting for HTTP 200 on ${baseUrl}`);
          await waitForHttpOk(baseUrl, { timeoutMs: 60_000, intervalMs: 1_000 });
          console.log("[migration] Old server ready");

          console.log("[migration] Publishing crates to old version");

          console.log("[migration] Publishing crate: test_lib");
          await publishCrate({
            cratePath: "tests/crates/test-migration/test_lib",
            registry,
            registryToken,
          });

          console.log("[migration] Publishing crate: foo-bar");
          await publishCrate({
            cratePath: "tests/crates/test-migration/foo-bar",
            registry,
            registryToken,
          });

          console.log("[migration] Published crates to old version");
        } finally {
          // Attach logs and stop container
          await attachContainerLogs(testInfo, startedOld.container, {
            name: startedOld.name,
            filePath: startedOld.logsFilePath,
          });
          startedOld.stopLogStreaming?.();
          await startedOld.container.stop().catch(() => { });
        }
      });

      // ---- Fix file ownership using Docker (no sudo needed) ----
      await test.step("fix data directory ownership", async () => {
        await fixDataDirOwnershipWithDocker(kdataDir);
      });

      // ---- Run new version (local) ----
      await test.step("run new version (local) and verify migration", async () => {
        // Start local Kellnr with the same data directory
        localKellnr = await startLocalKellnr({
          name: `kellnr-migration-new-${suffix}`,
          env: {
            // Use the existing kdata directory from the old container
            KELLNR_REGISTRY__DATA_DIR: kdataDir,
          },
        });

        console.log("[migration] New server (local) ready");

        console.log("[migration] Publishing crate to new version");
        await publishCrate({
          cratePath: "tests/crates/test-migration/full-toml",
          registry,
          registryToken,
        });
        console.log("[migration] Published crate to new version");
      });

      // ---- Verify UI accessibility after migration ----
      // Note: localKellnr is still running from the previous step
      await test.step("verify UI is accessible after migration", async () => {
        console.log("[migration] Starting UI verification tests");

        // Test 1: Landing page loads
        const landingPage = new LandingPage(page);
        await page.goto(baseUrl);
        await landingPage.waitForPageLoad();
        console.log("[migration] ✓ Landing page loaded");

        // Verify Kellnr branding is visible
        const hasBranding = await landingPage.hasKellnrBranding();
        expect(hasBranding).toBe(true);
        console.log("[migration] ✓ Kellnr branding visible");

        // Test 2: Statistics show migrated data
        await landingPage.waitForStatistics();
        const crateCount = await landingPage.getTotalCratesCount();
        expect(crateCount).toBe(3); // test_lib, foo-bar, full-toml
        console.log(`[migration] ✓ Statistics show ${crateCount} crates`);

        // Test 3: Crates page shows all migrated crates
        const cratesPage = new CratesPage(page);
        await page.goto(`${baseUrl}/crates`);
        await cratesPage.waitForSearchResults();

        const hasFooBar = await cratesPage.hasCrate("foo-bar");
        expect(hasFooBar).toBe(true);
        console.log("[migration] ✓ foo-bar crate visible");

        const hasTestLib = await cratesPage.hasCrate("test_lib");
        expect(hasTestLib).toBe(true);
        console.log("[migration] ✓ test_lib crate visible");

        const hasFullToml = await cratesPage.hasCrate("full-toml");
        expect(hasFullToml).toBe(true);
        console.log("[migration] ✓ full-toml crate visible");

        // Test 4: Crate details are accessible
        await page.goto(`${baseUrl}/crate?name=foo-bar`);
        const cratePage = new CratePage(page);
        await cratePage.waitForCrateData();

        const crateName = await cratePage.getCrateName();
        expect(crateName).toBe("foo-bar");

        const version = await cratePage.getVersion();
        expect(version).toBe("1.0.0");
        console.log("[migration] ✓ Crate details accessible");

        // Test 5: Dependencies are preserved
        await cratePage.clickTab("dependencies");
        await page.waitForTimeout(500);

        const testLibDep = page.locator(".dep-name").filter({ hasText: "test_lib" });
        await expect(testLibDep).toBeVisible();
        console.log("[migration] ✓ Dependencies preserved");

        console.log("[migration] All UI verification tests passed");
      });
    } finally {
      // Stop local Kellnr
      if (localKellnr) {
        try {
          console.log("[migration] Stopping local Kellnr");
          await localKellnr.stop();
        } catch {
          // best-effort
        }
      }

      // Cleanup kdata dir
      try {
        if (fileExists(kdataDir)) rimrafSync(kdataDir);
      } catch {
        // best-effort
      }
    }
  });
});
