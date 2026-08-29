/**
 * UI tests for GCS storage backend.
 *
 * Tests:
 * - Starts fake-gcs-server (GCS emulator) in Docker
 * - Starts local Kellnr with GCS backend enabled
 * - Publishes crates to GCS storage
 * - Verifies crates are visible and accessible in the UI
 *
 * Performance: Uses local Kellnr with Docker fake-gcs-server for GCS storage.
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
  buildGcsFakeServerImage,
  createNetwork,
  startGcsFakeServerContainer,
  type StartedGcs,
} from "./lib/docker";
import type { StartedNetwork } from "testcontainers";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";
import path from "node:path";

test.describe("GCS Storage UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;
  let network: StartedNetwork;
  let gcsContainer: StartedGcs;

  test.beforeAll(async ({}, testInfo) => {
    // Container + local setup needs more time
    test.setTimeout(15 * 60 * 1000); // 15 minutes for setup

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    await assertDockerAvailable();
    console.log("[setup] Docker is available");

    const suffix = `${Date.now()}`;
    const networkBaseName = `gcs-net-${suffix}`;
    const gcsBaseName = `fake-gcs-server-${suffix}`;

    const registry = "kellnr-test";

    // GCS settings. Auth is Application Default Credentials only; the emulator doesn't
    // validate requests, so kellnr talks to it via the endpoint-override + skip-signature path.
    const gcsImage = "kellnr-fake-gcs-storage";
    const gcsCratesBucket = "kellnr-crates";
    const gcsCratesioBucket = "kellnr-cratesio";

    // Extract registry token. The fixture crates are storage-backend agnostic (the token
    // is only used against kellnr's HTTP registry API), so we reuse the S3 fixture crates.
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

    console.log("[setup] Building fake-gcs-server image");
    await buildGcsFakeServerImage({
      imageName: gcsImage,
      cratesBucket: gcsCratesBucket,
      cratesioBucket: gcsCratesioBucket,
    });
    console.log("[setup] fake-gcs-server image built");

    gcsContainer = await startGcsFakeServerContainer(
      {
        name: gcsBaseName,
        image: gcsImage,
        network,
        exposeToHost: true, // Required for local Kellnr to access fake-gcs-server
      },
      testInfo,
    );

    console.log("[setup] fake-gcs-server container started");

    // Fixed host port reserved and baked into fake-gcs-server's `-public-host` up front
    // (see startGcsFakeServerContainer) so its flat object routes dispatch correctly.
    const gcsHostPort = gcsContainer.hostPort!;
    const gcsUrlForLocalKellnr = `http://localhost:${gcsHostPort}`;

    console.log(`[setup] fake-gcs-server accessible at ${gcsUrlForLocalKellnr}`);

    // Wait for fake-gcs-server to be fully ready by checking the bucket-listing endpoint
    console.log("[setup] Waiting for fake-gcs-server health check...");
    const healthUrl = `${gcsUrlForLocalKellnr}/storage/v1/b`;
    for (let i = 0; i < 30; i++) {
      try {
        const res = await fetch(healthUrl);
        if (res.ok) {
          console.log("[setup] fake-gcs-server health check passed");
          break;
        }
      } catch {
        // Not ready yet
      }
      await new Promise(resolve => setTimeout(resolve, 1000));
    }

    started = await startLocalKellnr({
      name: `kellnr-gcs-${suffix}`,
      env: {
        KELLNR_PROXY__ENABLED: "true",
        KELLNR_GCS__ENABLED: "true",
        KELLNR_GCS__ENDPOINT: gcsUrlForLocalKellnr,
        KELLNR_GCS__ALLOW_HTTP: "true",
        KELLNR_GCS__SKIP_SIGNATURE: "true",
        KELLNR_GCS__CRATES_BUCKET: gcsCratesBucket,
        KELLNR_GCS__CRATESIO_BUCKET: gcsCratesioBucket,
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl}`);

    console.log("[setup] Publishing crates to GCS storage");

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

    // Stop fake-gcs-server container
    if (gcsContainer) {
      try {
        console.log("[teardown] Stopping fake-gcs-server container");
        gcsContainer.stopLogStreaming?.();
        await gcsContainer.container.stop();
      } catch (e) {
        console.log("[teardown] Error stopping fake-gcs-server:", e);
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

  test("crates stored in GCS are visible in the UI", async ({ page }) => {
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

  test("crate details from GCS storage are accessible", async ({ page }) => {
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    // Verify crate name is displayed
    const crateName = await cratePage.getCrateName();
    expect(crateName).toBe("foo-bar");

    // Verify version is displayed
    const version = await cratePage.getVersion();
    expect(version).toBe("1.0.0");

    // Verify install snippet (stored in GCS) is available
    const installSnippet = await cratePage.getInstallSnippet();
    expect(installSnippet).toContain("foo-bar");
    expect(installSnippet).toContain("1.0.0");
  });

  test("crate dependencies from GCS storage are displayed", async ({ page }) => {
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

  test("search works with GCS-stored crates", async ({ page }) => {
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
