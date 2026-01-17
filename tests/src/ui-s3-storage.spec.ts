/**
 * UI tests for S3 storage backend.
 *
 * Tests:
 * - Starts MinIO (S3-compatible storage)
 * - Starts Kellnr with S3 backend enabled
 * - Publishes crates to S3 storage
 * - Verifies crates are visible and accessible in the UI
 *
 * Performance: Uses a single Kellnr + MinIO container setup.
 */

import { test, expect } from "./lib/ui-fixtures";
import { CratesPage, CratePage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  waitForHttpOk,
  assertDockerAvailable,
  publishCrate,
} from "./testUtils";
import { startKellnr, type StartedKellnr } from "./lib/kellnr";
import {
  buildS3MinioImage,
  createNetwork,
  startS3MinioContainer,
  type Started,
} from "./lib/docker";
import type { StartedNetwork } from "testcontainers";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";
import path from "node:path";

test.describe("S3 Storage UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedKellnr;
  let baseUrl: string;
  let network: StartedNetwork;
  let minioContainer: Started;

  test.beforeAll(async ({}, testInfo) => {
    // Container setup needs more time than default timeout
    test.setTimeout(15 * 60 * 1000); // 15 minutes for setup

    await assertDockerAvailable();
    console.log("[setup] Docker is available");

    const suffix = `${Date.now()}`;
    const networkBaseName = `s3-net-${suffix}`;
    const minioBaseName = `minio-${suffix}`;
    const kellnrBaseName = `kellnr-s3-${suffix}`;

    const kellnrImage = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";
    const registry = "kellnr-test";

    // S3 settings
    const s3RootUser = "minioadmin";
    const s3RootPassword = "minioadmin";
    const s3UrlInDockerNet = "http://minio:9000";
    const s3AllowHttp = "true";
    const s3Image = `custom-minio-${suffix}`;
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

    console.log("[setup] Building minio image");
    await buildS3MinioImage({
      imageName: s3Image,
      cratesBucket: s3CratesBucket,
      cratesioBucket: s3CratesioBucket,
    });
    console.log("[setup] Minio image built");

    minioContainer = await startS3MinioContainer(
      {
        name: minioBaseName,
        image: s3Image,
        network,
        rootUser: s3RootUser,
        rootPassword: s3RootPassword,
      },
      testInfo,
    );

    console.log("[setup] Minio container started");

    started = await startKellnr(
      {
        name: kellnrBaseName,
        image: kellnrImage,
        network,
        env: {
          KELLNR_PROXY__ENABLED: "true",
          KELLNR_S3__ENABLED: "true",
          KELLNR_S3__ACCESS_KEY: s3RootUser,
          KELLNR_S3__SECRET_KEY: s3RootPassword,
          KELLNR_S3__ENDPOINT: s3UrlInDockerNet,
          KELLNR_S3__ALLOW_HTTP: s3AllowHttp,
          KELLNR_S3__CRATES_BUCKET: s3CratesBucket,
          KELLNR_S3__CRATESIO_BUCKET: s3CratesioBucket,
        },
      },
      testInfo,
    );

    baseUrl = started.baseUrl;

    console.log(`[setup] Waiting for HTTP 200 on ${baseUrl}`);
    await waitForHttpOk(baseUrl, {
      timeoutMs: 60_000,
      intervalMs: 1_000,
    });
    console.log("[setup] Server ready");

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

    // Stop Kellnr container
    if (started) {
      try {
        console.log("[teardown] Stopping Kellnr container");
        await started.started.container.stop();
      } catch (e) {
        console.log("[teardown] Error stopping Kellnr:", e);
      }
    }

    // Stop MinIO container
    if (minioContainer) {
      try {
        console.log("[teardown] Stopping MinIO container");
        minioContainer.stopLogStreaming?.();
        await minioContainer.container.stop();
      } catch (e) {
        console.log("[teardown] Error stopping MinIO:", e);
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
    const testLibDep = page.locator("h3").filter({ hasText: "test_lib" });
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
