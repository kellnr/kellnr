/**
 * UI tests for S3 storage backend.
 *
 * Tests:
 * - Starts RustFS (S3-compatible storage) in Docker
 * - Starts local Kellnr with S3 backend enabled
 * - Publishes crates to S3 storage
 * - Verifies the crates are shown in the UI and downloadable from the storage
 *
 * List, search and dependencies read only the database and are tested with the
 * filesystem backend in ui-crate-with-data.spec.ts.
 *
 * Performance: Uses local Kellnr with Docker RustFS for S3 storage.
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
    // RustFS rejects its built-in default credentials on non-loopback
    // listeners since v1.0.0-alpha.85, use distinct values for the tests.
    const s3RootUser = "kellnr-test-access";
    const s3RootPassword = "kellnr-test-secret-1234567890";
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

    // startS3RustFsContainer already waits for its listening port; the tests exercise the buckets.
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

    // Verify install snippet is shown
    const installSnippet = await cratePage.getInstallSnippet();
    expect(installSnippet).toContain("foo-bar");
    expect(installSnippet).toContain("1.0.0");
  });

  test("published crates are downloadable from S3 storage", async () => {
    for (const crate of PUBLISHED_STORAGE_TEST_CRATES) {
      await expectCrateDownloadMatchesIndex(baseUrl, crate);
    }
  });
});
