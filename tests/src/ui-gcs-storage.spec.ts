/**
 * UI tests for GCS storage backend.
 *
 * Tests:
 * - Starts fake-gcs-server (GCS emulator) in Docker
 * - Starts local Kellnr with GCS backend enabled
 * - Publishes crates to GCS storage
 * - Verifies the crates are shown in the UI and downloadable from the storage
 *
 * List, search and dependencies read only the database and are tested with the
 * filesystem backend in ui-crate-with-data.spec.ts.
 *
 * Performance: Uses local Kellnr with Docker fake-gcs-server for GCS storage.
 */

import { test, expect } from "./lib/ui-fixtures";
import { CratePage } from "./pages";
import { expectCrateDownloadMatchesIndex, PUBLISHED_STORAGE_TEST_CRATES } from "./lib/storage-checks";
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

    // startGcsFakeServerContainer already waits for the bucket-listing endpoint.
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

    // Verify install snippet is shown
    const installSnippet = await cratePage.getInstallSnippet();
    expect(installSnippet).toContain("foo-bar");
    expect(installSnippet).toContain("1.0.0");
  });

  test("published crates are downloadable from GCS storage", async () => {
    for (const crate of PUBLISHED_STORAGE_TEST_CRATES) {
      await expectCrateDownloadMatchesIndex(baseUrl, crate);
    }
  });
});
