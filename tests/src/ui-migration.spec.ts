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
import { execSync } from "node:child_process";

function rimrafSync(p: string) {
  fs.rmSync(p, { recursive: true, force: true });
}

/**
 * Debug helper: print directory contents with permissions and ownership.
 */
function debugPrintDirContents(dirPath: string, label: string): void {
  console.log(`\n[DEBUG] === ${label} ===`);
  console.log(`[DEBUG] Directory: ${dirPath}`);

  try {
    // Check if directory exists
    if (!fs.existsSync(dirPath)) {
      console.log(`[DEBUG] Directory does not exist!`);
      return;
    }

    // Get directory stats
    const dirStat = fs.statSync(dirPath);
    console.log(`[DEBUG] Dir mode: ${dirStat.mode.toString(8)}, uid: ${dirStat.uid}, gid: ${dirStat.gid}`);

    // List contents with ls -la
    try {
      const lsOutput = execSync(`ls -la "${dirPath}"`, { encoding: "utf-8" });
      console.log(`[DEBUG] Contents:\n${lsOutput}`);
    } catch (e) {
      console.log(`[DEBUG] ls -la failed: ${e}`);
    }

    // Recursively list all files
    try {
      const findOutput = execSync(`find "${dirPath}" -type f | head -20`, { encoding: "utf-8" });
      console.log(`[DEBUG] Files (first 20):\n${findOutput}`);
    } catch (e) {
      console.log(`[DEBUG] find failed: ${e}`);
    }

    // Show ownership of first few files
    try {
      const lsRecursive = execSync(`ls -laR "${dirPath}" | head -50`, { encoding: "utf-8" });
      console.log(`[DEBUG] Recursive listing (first 50 lines):\n${lsRecursive}`);
    } catch (e) {
      console.log(`[DEBUG] ls -laR failed: ${e}`);
    }
  } catch (e) {
    console.log(`[DEBUG] Error inspecting directory: ${e}`);
  }
  console.log(`[DEBUG] === END ${label} ===\n`);
}

/**
 * Debug helper: print current user info.
 */
function debugPrintUserInfo(): void {
  console.log(`\n[DEBUG] === User Info ===`);

  const uid = typeof process.getuid === "function" ? process.getuid() : null;
  const gid = typeof process.getgid === "function" ? process.getgid() : null;
  console.log(`[DEBUG] process.getuid(): ${uid}`);
  console.log(`[DEBUG] process.getgid(): ${gid}`);

  try {
    const whoami = execSync("whoami", { encoding: "utf-8" }).trim();
    console.log(`[DEBUG] whoami: ${whoami}`);
  } catch (e) {
    console.log(`[DEBUG] whoami failed: ${e}`);
  }

  try {
    const id = execSync("id", { encoding: "utf-8" }).trim();
    console.log(`[DEBUG] id: ${id}`);
  } catch (e) {
    console.log(`[DEBUG] id failed: ${e}`);
  }

  try {
    const sudoCheck = execSync("sudo -n true 2>&1 && echo 'sudo: YES' || echo 'sudo: NO'", { encoding: "utf-8", shell: "/bin/bash" }).trim();
    console.log(`[DEBUG] ${sudoCheck}`);
  } catch (e) {
    console.log(`[DEBUG] sudo check failed: ${e}`);
  }

  console.log(`[DEBUG] === END User Info ===\n`);
}

/**
 * Fix ownership of files in a directory after Docker has written to them.
 * Docker runs as root, so files are owned by root. This uses sudo to change
 * ownership to the current user so the local Kellnr process can access them.
 *
 * This works on GitHub Actions runners which have passwordless sudo.
 * On systems without sudo or where sudo requires a password, this will fail
 * silently and the test will fail with a more specific error later.
 */
function fixDataDirOwnership(dirPath: string): void {
  console.log(`[DEBUG] fixDataDirOwnership called for: ${dirPath}`);

  // Get current user's UID:GID
  const uid = typeof process.getuid === "function" ? process.getuid() : null;
  const gid = typeof process.getgid === "function" ? process.getgid() : null;

  console.log(`[DEBUG] Target UID:GID = ${uid}:${gid}`);

  if (uid === null || gid === null) {
    console.log("[migration] Cannot determine current user UID/GID, skipping ownership fix");
    return;
  }

  try {
    // Use sudo to change ownership recursively
    const cmd = `sudo chown -R ${uid}:${gid} "${dirPath}"`;
    console.log(`[DEBUG] Running: ${cmd}`);
    execSync(cmd, { stdio: "inherit" });
    console.log(`[migration] Changed ownership to ${uid}:${gid}`);
  } catch (e) {
    // sudo might not be available or might require a password
    console.log(`[DEBUG] sudo chown failed with error: ${e}`);
    console.log("[migration] Warning: Could not fix data directory ownership (sudo may not be available)");
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
        // Debug: Print user info at the start
        debugPrintUserInfo();

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
            env: k.env,
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

      // ---- Fix file ownership after Docker ----
      await test.step("fix data directory ownership", async () => {
        console.log("[migration] Fixing data directory ownership after Docker");

        // Debug: Show directory contents BEFORE ownership fix
        debugPrintDirContents(kdataDir, "BEFORE ownership fix");

        fixDataDirOwnership(kdataDir);

        // Debug: Show directory contents AFTER ownership fix
        debugPrintDirContents(kdataDir, "AFTER ownership fix");
      });

      // ---- Run new version (local) ----
      await test.step("run new version (local) and verify migration", async () => {
        console.log("[DEBUG] About to start local Kellnr process...");
        console.log(`[DEBUG] Using data directory: ${kdataDir}`);

        // Start local Kellnr with the same data directory
        try {
          localKellnr = await startLocalKellnr({
            name: `kellnr-migration-new-${suffix}`,
            env: {
              // Use the existing kdata directory from the old container
              KELLNR_REGISTRY__DATA_DIR: kdataDir,
            },
          });
          console.log("[migration] New server (local) ready");
        } catch (e) {
          console.log(`[DEBUG] startLocalKellnr FAILED: ${e}`);
          // Print directory state at failure time
          debugPrintDirContents(kdataDir, "AT FAILURE TIME");
          throw e;
        }

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
