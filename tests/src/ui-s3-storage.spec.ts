/**
 * UI tests for S3 storage backend.
 *
 * Tests:
 * - Starts RustFS (S3-compatible storage) in Docker
 * - Starts local Kellnr with S3 backend enabled
 * - Publishes crates to S3 storage
 * - Verifies crates are visible and accessible in the UI
 *
 * Performance: Uses local Kellnr with Docker RustFS for S3 storage.
 */

import { test, expect } from "./lib/ui-fixtures";
import { CratesPage, CratePage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  assertKellnrBinaryExists,
  assertDockerAvailable,
  publishCrate,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import {
  buildS3RustFsImage,
  createNetwork,
  startS3RustFsContainer,
  type Started,
} from "./lib/docker";
import type { StartedNetwork } from "testcontainers";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";
import path from "node:path";

test.describe("S3 Storage UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;
  let network: StartedNetwork;
  let rustfsContainer: Started;

  test.beforeAll(async ({}, testInfo) => {
    // Container + local setup needs more time
    test.setTimeout(15 * 60 * 1000); // 15 minutes for setup

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    await assertDockerAvailable();
    console.log("[setup] Docker is available");

    const suffix = `${Date.now()}`;
    const networkBaseName = `s3-net-${suffix}`;
    const rustfsBaseName = `rustfs-${suffix}`;

    const registry = "kellnr-test";

    // S3 settings
    const s3RootUser = "rustfsadmin";
    const s3RootPassword = "rustfsadmin";
    const s3AllowHttp = "true";
    const s3Image = "kellnr-rustfs-storage";
    const s3CratesBucket = "kellnr-crates";
    const s3CratesioBucket = "kellnr-cratesio";

    // Extract registry token
    const tokenSourceCrateDir = path.resolve(
      process.cwd(),
      "crates",
      "test-s3-storage",
      "foo-bar",
    );
    const registryToken = extractRegistryTokenFromCargoConfig({
      crateDir: tokenSourceCrateDir,
      registryName: registry,
    });

    network = await createNetwork(networkBaseName, testInfo);

    console.log("[setup] Building RustFS image");
    await buildS3RustFsImage({
      imageName: s3Image,
      cratesBucket: s3CratesBucket,
      cratesioBucket: s3CratesioBucket,
    });
    console.log("[setup] RustFS image built");

    rustfsContainer = await startS3RustFsContainer(
      {
        name: rustfsBaseName,
        image: s3Image,
        network,
        rootUser: s3RootUser,
        rootPassword: s3RootPassword,
        exposeToHost: true, // Required for local Kellnr to access RustFS
      },
      testInfo,
    );

    console.log("[setup] RustFS container started");

    // Get RustFS's mapped host port for localhost access
    const rustfsHostPort = rustfsContainer.container.getMappedPort(9000);
    const s3UrlForLocalKellnr = `http://localhost:${rustfsHostPort}`;

    console.log(`[setup] RustFS accessible at ${s3UrlForLocalKellnr}`);

    // Wait for RustFS to be fully ready by checking the health endpoint
    console.log("[setup] Waiting for RustFS health check...");
    const healthUrl = `${s3UrlForLocalKellnr}/health/live`;
    for (let i = 0; i < 30; i++) {
      try {
        const res = await fetch(healthUrl);
        if (res.ok) {
          console.log("[setup] RustFS health check passed");
          break;
        }
      } catch {
        // Not ready yet
      }
      await new Promise(resolve => setTimeout(resolve, 1000));
    }

    started = await startLocalKellnr({
      name: `kellnr-s3-${suffix}`,
      env: {
        KELLNR_PROXY__ENABLED: "true",
        KELLNR_S3__ENABLED: "true",
        KELLNR_S3__ACCESS_KEY: s3RootUser,
        KELLNR_S3__SECRET_KEY: s3RootPassword,
        KELLNR_S3__ENDPOINT: s3UrlForLocalKellnr,
        KELLNR_S3__ALLOW_HTTP: s3AllowHttp,
        KELLNR_S3__CRATES_BUCKET: s3CratesBucket,
        KELLNR_S3__CRATESIO_BUCKET: s3CratesioBucket,
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl}`);

    console.log("[setup] Publishing crates to S3 storage");

    console.log("[setup] Publishing crate: test_lib");
    await publishCrate({
      cratePath: "tests/crates/test-s3-storage/test_lib",
      registry,
      registryToken,
    });

    console.log("[setup] Publishing crate: UpperCase-Name123");
    await publishCrate({
      cratePath: "tests/crates/test-s3-storage/UpperCase-Name123",
      registry,
      registryToken,
    });

    console.log("[setup] Publishing crate: foo-bar");
    await publishCrate({
      cratePath: "tests/crates/test-s3-storage/foo-bar",
      registry,
      registryToken,
    });

    console.log("[setup] Crate publishing finished");
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    console.log("[teardown] Starting cleanup");

    // Stop Kellnr process
    if (started) {
      try {
        console.log("[teardown] Stopping Kellnr process");
        await started.stop();
      } catch (e) {
        console.log("[teardown] Error stopping Kellnr:", e);
      }
    }

    // Stop RustFS container
    if (rustfsContainer) {
      try {
        console.log("[teardown] Stopping RustFS container");
        rustfsContainer.stopLogStreaming?.();
        await rustfsContainer.container.stop();
      } catch (e) {
        console.log("[teardown] Error stopping RustFS:", e);
      }
    }

    // Stop network
    if (network) {
      try {
        console.log("[teardown] Stopping network");
        await network.stop();
      } catch (e) {
        console.log("[teardown] Error stopping network:", e);
      }
    }

    console.log("[teardown] Cleanup complete");
  });

  test("crates stored in S3 are visible in the UI", async ({ page }) => {
    const cratesPage = new CratesPage(page);

    await page.goto(`${baseUrl}/crates`);
    await cratesPage.waitForPageLoad();
    await cratesPage.waitForSearchResults();

    // Verify all three crates are visible
    const hasFooBar = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBar).toBe(true);

    const hasTestLib = await cratesPage.hasCrate("test_lib");
    expect(hasTestLib).toBe(true);

    const hasUpperCase = await cratesPage.hasCrate("UpperCase-Name123");
    expect(hasUpperCase).toBe(true);
  });

  test("crate details from S3 storage are accessible", async ({ page }) => {
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    // Verify crate name is displayed
    const crateName = await cratePage.getCrateName();
    expect(crateName).toBe("foo-bar");

    // Verify version is displayed
    const version = await cratePage.getVersion();
    expect(version).toBe("1.0.0");

    // Verify install snippet (stored in S3) is available
    const installSnippet = await cratePage.getInstallSnippet();
    expect(installSnippet).toContain("foo-bar");
    expect(installSnippet).toContain("1.0.0");
  });

  test("crate dependencies from S3 storage are displayed", async ({ page }) => {
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    // Click on dependencies tab
    await cratePage.clickTab("dependencies");
    await page.waitForTimeout(500);

    // Verify test_lib dependency is shown
    const testLibDep = page.locator(".dep-name").filter({ hasText: "test_lib" });
    await expect(testLibDep).toBeVisible();
  });

  test("search works with S3-stored crates", async ({ page }) => {
    const cratesPage = new CratesPage(page);

    await page.goto(`${baseUrl}/crates`);
    await cratesPage.waitForPageLoad();

    // Search for foo-bar
    await cratesPage.searchInput.fill("foo");
    await cratesPage.searchInput.press("Enter");
    await cratesPage.waitForSearchResults();

    // Verify foo-bar is in search results
    const hasFooBar = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBar).toBe(true);
  });
});
