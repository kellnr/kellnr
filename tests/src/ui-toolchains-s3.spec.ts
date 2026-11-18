/**
 * Storage tests for toolchains with the S3 backend.
 *
 * Tests:
 * - Starts RustFS (S3-compatible storage) in Docker
 * - Starts local Kellnr with S3 backend and toolchain feature enabled
 * - Uploads toolchains to S3 storage via API
 * - Verifies the archives are split into components, downloadable and deleted
 *
 * The list, channel and UI views read only the database and are tested with the
 * filesystem backend in ui-toolchains.spec.ts.
 *
 * Performance: Uses local Kellnr with Docker RustFS for S3 storage.
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
  buildS3RustFsImage,
  createNetwork,
  startS3RustFsContainer,
  type Started,
} from "./lib/docker";
import type { StartedNetwork } from "testcontainers";
import fs from "node:fs";
import path from "node:path";

test.describe("Toolchain S3 Storage Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;
  let network: StartedNetwork;
  let rustfsContainer: Started;
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

  // S3 settings
  // RustFS rejects its built-in default credentials on non-loopback
  // listeners since v1.0.0-alpha.85, use distinct values for the tests.
  const s3RootUser = "kellnr-test-access";
  const s3RootPassword = "kellnr-test-secret-1234567890";
  const s3CratesBucket = "kellnr-crates";
  const s3CratesioBucket = "kellnr-cratesio";
  const s3ToolchainBucket = "kellnr-toolchains";

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
    const networkBaseName = `s3-toolchain-net-${suffix}`;
    const rustfsBaseName = `rustfs-toolchain-${suffix}`;
    // Use a consistent image name to avoid rebuilding on every test run
    const s3Image = "kellnr-rustfs-toolchains";

    network = await createNetwork(networkBaseName, testInfo);

    console.log("[setup] Building RustFS image with toolchain bucket");
    await buildS3RustFsImage({
      imageName: s3Image,
      cratesBucket: s3CratesBucket,
      cratesioBucket: s3CratesioBucket,
      toolchainBucket: s3ToolchainBucket,
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

    // startS3RustFsContainer already waits for its listening port; the tests exercise the buckets.
    started = await startLocalKellnr({
      name: `kellnr-toolchain-s3-${suffix}`,
      logLevel: "info",
      webLogLevel: "info",
      logToStdout: true, // Log to stdout for debugging
      env: {
        KELLNR_REGISTRY__AUTH_REQUIRED: "true",
        KELLNR_TOOLCHAIN__ENABLED: "true",
        KELLNR_S3__ENABLED: "true",
        KELLNR_S3__ACCESS_KEY: s3RootUser,
        KELLNR_S3__SECRET_KEY: s3RootPassword,
        KELLNR_S3__ENDPOINT: s3UrlForLocalKellnr,
        KELLNR_S3__ALLOW_HTTP: "true",
        KELLNR_S3__CRATES_BUCKET: s3CratesBucket,
        KELLNR_S3__CRATESIO_BUCKET: s3CratesioBucket,
        KELLNR_S3__TOOLCHAIN_BUCKET: s3ToolchainBucket,
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

  test("uploaded toolchain targets are split and downloadable from S3 storage", async () => {
    // Two targets of one toolchain share the date prefix 2024-01-15/ in the bucket
    await uploadToolchain(baseUrl, sessionCookie, x86);
    await uploadToolchain(baseUrl, sessionCookie, arm);

    const manifest = await waitForStableManifest(baseUrl, sessionCookie, [x86.target, arm.target]);
    for (const upload of [x86, arm]) {
      await expectToolchainDownloadMatches(toolchainArchiveUrl(manifest, upload.target), sessionCookie, upload.archivePath);
      await expectDownloadStatus(toolchainArchiveUrl(manifest, upload.target, "rustc"), sessionCookie, 200);
    }
  });

  test("deleting a toolchain target removes its archives from S3 storage", async () => {
    const manifest = await waitForStableManifest(baseUrl, sessionCookie, [x86.target, arm.target]);

    await deleteToolchainTarget(baseUrl, sessionCookie, arm.target);

    // The download reads the storage, not the database, so a 404 shows the archive was deleted
    await expectDownloadStatus(toolchainArchiveUrl(manifest, arm.target), sessionCookie, 404);
    await expectDownloadStatus(toolchainArchiveUrl(manifest, arm.target, "rustc"), sessionCookie, 404);
    await expectToolchainDownloadMatches(toolchainArchiveUrl(manifest, x86.target), sessionCookie, x86.archivePath);
  });
});
