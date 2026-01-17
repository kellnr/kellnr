/**
 * UI tests for database migration and version upgrades.
 *
 * Tests:
 * - Starts Kellnr with old version and publishes crates
 * - Stops old container
 * - Starts Kellnr with new version (same data directory)
 * - Verifies database migration succeeded
 * - Verifies UI is accessible and shows migrated data
 *
 * Performance: Sequential test with old → new version upgrade.
 */

import { test, expect } from "./lib/ui-fixtures";
import { LandingPage, CratesPage, CratePage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  waitForHttpOk,
  assertDockerAvailable,
  publishCrate,
  fetchLatestReleasedKellnrImage,
  ensureLocalKellnrTestImage,
} from "./testUtils";
import { startContainer, attachContainerLogs, type Started } from "./lib/docker";
import { kellnrDefaults } from "./lib/kellnr";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";
import fs from "node:fs";
import path from "node:path";

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

test.describe("Migration UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let kdataDir: string;
  let baseUrl: string;

  test("migrates from old version to new and UI is accessible", async ({ page }, testInfo) => {
    testInfo.setTimeout(20 * 60 * 1000); // 20 minutes

    // Get old and new images
    const oldImageFromEnv = process.env.KELLNR_OLD_IMAGE;
    const newImage = process.env.KELLNR_NEW_IMAGE;

    const resolvedNewImage =
      newImage ?? process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";

    const oldImage =
      oldImageFromEnv && oldImageFromEnv.trim() !== ""
        ? oldImageFromEnv
        : await fetchLatestReleasedKellnrImage();

    console.log(`[migration] Old image: ${oldImage}`);
    console.log(`[migration] New image: ${resolvedNewImage}`);

    const registry = process.env.KELLNR_MIGRATION_REGISTRY ?? "kellnr-local";
    const suffix = `${Date.now()}`;
    const oldContainer = `kellnr-old-${suffix}`;
    const newContainer = `kellnr-new-${suffix}`;

    // Fixed localhost:8000 setup
    const k = kellnrDefaults();
    baseUrl = k.baseUrl;

    // Persisted data directory
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

    try {
      await test.step("check prerequisites", async () => {
        await assertDockerAvailable();
        console.log("[migration] Docker is available");
      });

      await test.step("ensure new image exists", async () => {
        await ensureLocalKellnrTestImage(resolvedNewImage);
        console.log("[migration] New image is available");
      });

      await test.step("prepare kdata dir", async () => {
        console.log(`[migration] Preparing kdata dir: ${kdataDir}`);
        rimrafSync(kdataDir);
        fs.mkdirSync(kdataDir, { recursive: true });
        console.log("[migration] kdata dir ready");
      });

      // ---- Run old container ----
      await test.step("run old version and publish crates", async () => {
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

      // ---- Run new container ----
      await test.step("run new version and verify migration", async () => {
        const startedNew = await startContainer(
          {
            name: newContainer,
            image: resolvedNewImage,
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
          console.log("[migration] New server ready");

          console.log("[migration] Publishing crate to new version");
          await publishCrate({
            cratePath: "tests/crates/test-migration/full-toml",
            registry,
            registryToken,
          });
          console.log("[migration] Published crate to new version");
        } finally {
          // Attach logs and stop container
          await attachContainerLogs(testInfo, startedNew.container, {
            name: startedNew.name,
            filePath: startedNew.logsFilePath,
          });
          startedNew.stopLogStreaming?.();
          await startedNew.container.stop().catch(() => { });
        }
      });

      // ---- Verify UI accessibility after migration ----
      await test.step("verify UI is accessible after migration", async () => {
        // Start the new container again for UI testing
        const startedNewForUI = await startContainer(
          {
            name: `${newContainer}-ui`,
            image: resolvedNewImage,
            ports: k.ports,
            env: k.env,
            bindMounts: {
              [kdataDir]: kdataMount,
            },
          },
          testInfo,
        );

        try {
          console.log("[migration] Waiting for new server for UI tests");
          await waitForHttpOk(baseUrl, { timeoutMs: 60_000, intervalMs: 1_000 });
          console.log("[migration] New server ready for UI tests");

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

          const testLibDep = page.locator("h3").filter({ hasText: "test_lib" });
          await expect(testLibDep).toBeVisible();
          console.log("[migration] ✓ Dependencies preserved");

          console.log("[migration] All UI verification tests passed");
        } finally {
          // Attach logs and stop container
          await attachContainerLogs(testInfo, startedNewForUI.container, {
            name: startedNewForUI.name,
            filePath: startedNewForUI.logsFilePath,
          });
          startedNewForUI.stopLogStreaming?.();
          await startedNewForUI.container.stop().catch(() => { });
        }
      });
    } finally {
      // Cleanup kdata dir
      try {
        if (fileExists(kdataDir)) rimrafSync(kdataDir);
      } catch {
        // best-effort
      }
    }
  });
});
