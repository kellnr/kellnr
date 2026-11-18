/**
 * Storage tests for toolchains with the GCS backend.
 *
 * Tests:
 * - Starts fake-gcs-server (GCS emulator) in Docker
 * - Starts local Kellnr with GCS backend and toolchain feature enabled
 * - Uploads toolchains to GCS storage via API
 * - Verifies the archives are split into components, downloadable and deleted
 *
 * The list, channel and UI views read only the database and are tested with the
 * filesystem backend in ui-toolchains.spec.ts.
 *
 * Performance: Uses local Kellnr with Docker fake-gcs-server for GCS storage.
 */

import { test, expect } from "./lib/ui-fixtures";
import {
  deleteToolchainTarget,
  expectDownloadStatus,
  expectToolchainDownloadMatches,
  loginAsAdminViaApi,
  toolchainArchiveUrl,
  uploadToolchain,
  waitForStableManifest,
} from "./lib/storage-checks";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  assertKellnrBinaryExists,
  assertDockerAvailable,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import {
  buildGcsFakeServerImage,
  createNetwork,
  startGcsFakeServerContainer,
  type StartedGcs,
} from "./lib/docker";
import type { StartedNetwork } from "testcontainers";
import fs from "node:fs";
import path from "node:path";

test.describe("Toolchain GCS Storage Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;
  let network: StartedNetwork;
  let gcsContainer: StartedGcs;
  let sessionCookie: string;

  // Paths to test archives (both targets for architecture-independent testing)
  const fixturesDir = path.resolve(process.cwd(), "fixtures", "test-toolchain");
  const x86 = {
    target: "x86_64-unknown-linux-gnu",
    archivePath: path.resolve(fixturesDir, "rust-1.0.0-test-x86_64-unknown-linux-gnu.tar.xz"),
  };
  const arm = {
    target: "aarch64-unknown-linux-gnu",
    archivePath: path.resolve(fixturesDir, "rust-1.0.0-test-aarch64-unknown-linux-gnu.tar.xz"),
  };

  // GCS settings
  const gcsCratesBucket = "kellnr-crates";
  const gcsCratesioBucket = "kellnr-cratesio";
  const gcsToolchainBucket = "kellnr-toolchains";

  test.beforeAll(async ({}, testInfo) => {
    // Container + local setup needs more time
    test.setTimeout(15 * 60 * 1000); // 15 minutes for setup

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    await assertDockerAvailable();
    console.log("[setup] Docker is available");

    // Verify test archive exists
    for (const { archivePath } of [x86, arm]) {
      if (!fs.existsSync(archivePath)) {
        throw new Error(`Test archive not found at ${archivePath}. Run create-test-archive.sh first.`);
      }
    }

    const suffix = `${Date.now()}`;
    const networkBaseName = `gcs-toolchain-net-${suffix}`;
    const gcsBaseName = `fake-gcs-server-toolchain-${suffix}`;
    // Use a consistent image name to avoid rebuilding on every test run
    const gcsImage = "kellnr-fake-gcs-toolchains";

    network = await createNetwork(networkBaseName, testInfo);

    console.log("[setup] Building fake-gcs-server image with toolchain bucket");
    await buildGcsFakeServerImage({
      imageName: gcsImage,
      cratesBucket: gcsCratesBucket,
      cratesioBucket: gcsCratesioBucket,
      toolchainBucket: gcsToolchainBucket,
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
      name: `kellnr-toolchain-gcs-${suffix}`,
      logLevel: "info",
      webLogLevel: "info",
      logToStdout: true, // Log to stdout for debugging
      env: {
        KELLNR_REGISTRY__AUTH_REQUIRED: "true",
        KELLNR_TOOLCHAIN__ENABLED: "true",
        KELLNR_GCS__ENABLED: "true",
        KELLNR_GCS__ENDPOINT: gcsUrlForLocalKellnr,
        KELLNR_GCS__ALLOW_HTTP: "true",
        KELLNR_GCS__SKIP_SIGNATURE: "true",
        KELLNR_GCS__CRATES_BUCKET: gcsCratesBucket,
        KELLNR_GCS__CRATESIO_BUCKET: gcsCratesioBucket,
        KELLNR_GCS__TOOLCHAIN_BUCKET: gcsToolchainBucket,
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl}`);
    sessionCookie = await loginAsAdminViaApi(baseUrl);
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

  test("uploaded toolchain targets are split and downloadable from GCS storage", async () => {
    // Two targets of one toolchain share the date prefix 2024-01-15/ in the bucket
    await uploadToolchain(baseUrl, sessionCookie, x86);
    await uploadToolchain(baseUrl, sessionCookie, arm);

    const manifest = await waitForStableManifest(baseUrl, sessionCookie, [x86.target, arm.target]);
    for (const upload of [x86, arm]) {
      await expectToolchainDownloadMatches(toolchainArchiveUrl(manifest, upload.target), sessionCookie, upload.archivePath);
      await expectDownloadStatus(toolchainArchiveUrl(manifest, upload.target, "rustc"), sessionCookie, 200);
    }
  });

  test("deleting a toolchain target removes its archives from GCS storage", async () => {
    const manifest = await waitForStableManifest(baseUrl, sessionCookie, [x86.target, arm.target]);

    await deleteToolchainTarget(baseUrl, sessionCookie, arm.target);

    // The download reads the storage, not the database, so a 404 shows the archive was deleted
    await expectDownloadStatus(toolchainArchiveUrl(manifest, arm.target), sessionCookie, 404);
    await expectDownloadStatus(toolchainArchiveUrl(manifest, arm.target, "rustc"), sessionCookie, 404);
    await expectToolchainDownloadMatches(toolchainArchiveUrl(manifest, x86.target), sessionCookie, x86.archivePath);
  });
});
