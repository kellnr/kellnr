/**
 * UI tests for toolchain management with S3 storage backend.
 *
 * Tests:
 * - Starts MinIO (S3-compatible storage) in Docker
 * - Starts local Kellnr with S3 backend and toolchain feature enabled
 * - Uploads toolchains to S3 storage via API
 * - Verifies toolchains are accessible and manifests are generated correctly
 *
 * Performance: Uses local Kellnr with Docker MinIO for S3 storage.
 */

import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { LoginPage, ToolchainsPage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  assertKellnrBinaryExists,
  assertDockerAvailable,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import {
  buildS3MinioImage,
  createNetwork,
  startS3MinioContainer,
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
  let minioContainer: Started;
  let sessionCookie: string;

  // Path to test archive
  const testArchivePath = path.resolve(
    process.cwd(),
    "fixtures",
    "test-toolchain",
    "rust-1.0.0-test-x86_64-unknown-linux-gnu.tar.xz"
  );

  // S3 settings
  const s3RootUser = "minioadmin";
  const s3RootPassword = "minioadmin";
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
    if (!fs.existsSync(testArchivePath)) {
      throw new Error(`Test archive not found at ${testArchivePath}. Run create-test-archive.sh first.`);
    }
    console.log(`[setup] Test archive found at ${testArchivePath}`);

    const suffix = `${Date.now()}`;
    const networkBaseName = `s3-toolchain-net-${suffix}`;
    const minioBaseName = `minio-toolchain-${suffix}`;
    // Use a consistent image name to avoid rebuilding on every test run
    const s3Image = "kellnr-minio-toolchains";

    network = await createNetwork(networkBaseName, testInfo);

    console.log("[setup] Building minio image with toolchain bucket");
    await buildS3MinioImage({
      imageName: s3Image,
      cratesBucket: s3CratesBucket,
      cratesioBucket: s3CratesioBucket,
      toolchainBucket: s3ToolchainBucket,
    });
    console.log("[setup] Minio image built");

    minioContainer = await startS3MinioContainer(
      {
        name: minioBaseName,
        image: s3Image,
        network,
        rootUser: s3RootUser,
        rootPassword: s3RootPassword,
        exposeToHost: true, // Required for local Kellnr to access MinIO
      },
      testInfo,
    );

    console.log("[setup] Minio container started");

    // Get MinIO's mapped host port for localhost access
    const minioHostPort = minioContainer.container.getMappedPort(9000);
    const s3UrlForLocalKellnr = `http://localhost:${minioHostPort}`;

    console.log(`[setup] MinIO accessible at ${s3UrlForLocalKellnr}`);

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

  test("can upload toolchain to S3 storage via API", async ({ page }) => {
    // First login to get a session cookie
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(500);

    // Get session cookie from browser context
    const cookies = await page.context().cookies();
    const session = cookies.find(c => c.name === "kellnr_session_id");
    expect(session).toBeDefined();
    sessionCookie = `kellnr_session_id=${session!.value}`;

    // Read the test archive
    const archiveData = fs.readFileSync(testArchivePath);

    // Upload via API
    const uploadUrl = `${baseUrl}/api/v1/toolchains?name=rust&version=1.0.0-s3-test&target=x86_64-unknown-linux-gnu&date=2024-01-15&channel=stable`;

    const response = await fetch(uploadUrl, {
      method: "PUT",
      headers: {
        "Content-Type": "application/octet-stream",
        "Cookie": sessionCookie,
      },
      body: archiveData,
    });

    if (response.status !== 200) {
      const errorBody = await response.text();
      console.log(`[test] Upload failed with status ${response.status}: ${errorBody}`);
    }
    expect(response.status).toBe(200);
    const result = await response.json();
    expect(result.success).toBe(true);
    console.log(`[test] Upload response: ${JSON.stringify(result)}`);
  });

  test("can fetch channel manifest from S3-backed toolchain", async () => {
    // The manifest should be available after upload
    const manifestUrl = `${baseUrl}/api/v1/toolchains/dist/channel-rust-stable.toml`;

    const response = await fetch(manifestUrl, {
      headers: { "Cookie": sessionCookie },
    });
    expect(response.status).toBe(200);

    const manifest = await response.text();
    console.log(`[test] Manifest content:\n${manifest}`);

    // Verify manifest structure
    expect(manifest).toContain('manifest-version = "2"');
    expect(manifest).toContain('[pkg.rust]');
    expect(manifest).toContain('version = "1.0.0-s3-test"');
    expect(manifest).toContain('x86_64-unknown-linux-gnu');
    expect(manifest).toContain('available = true');
    expect(manifest).toContain('url = "');
    expect(manifest).toContain('hash = "');
  });

  test("can download toolchain archive from S3 storage", async () => {
    // First get the manifest to find the archive URL
    const manifestUrl = `${baseUrl}/api/v1/toolchains/dist/channel-rust-stable.toml`;
    const manifestResponse = await fetch(manifestUrl, {
      headers: { "Cookie": sessionCookie },
    });
    const manifest = await manifestResponse.text();

    // Extract URL from manifest
    const urlMatch = manifest.match(/url = "([^"]+)"/);
    expect(urlMatch).toBeDefined();
    const archiveUrl = urlMatch![1];
    console.log(`[test] Archive URL: ${archiveUrl}`);

    // Download the archive
    const response = await fetch(archiveUrl, {
      headers: { "Cookie": sessionCookie },
    });
    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toBe("application/x-xz");

    const data = await response.arrayBuffer();
    expect(data.byteLength).toBeGreaterThan(0);
    console.log(`[test] Downloaded archive size: ${data.byteLength} bytes`);

    // Verify it's the same size as our original archive
    const originalSize = fs.statSync(testArchivePath).size;
    expect(data.byteLength).toBe(originalSize);
  });

  test("toolchain stored in S3 appears in list API", async ({ page }) => {
    // Login to get session
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(500);

    const cookies = await page.context().cookies();
    const session = cookies.find(c => c.name === "kellnr_session_id");
    const cookie = `kellnr_session_id=${session!.value}`;

    // List toolchains via API
    const listUrl = `${baseUrl}/api/v1/toolchains`;
    const response = await fetch(listUrl, {
      headers: { "Cookie": cookie },
    });

    expect(response.status).toBe(200);
    const toolchains = await response.json();
    console.log(`[test] Toolchains list: ${JSON.stringify(toolchains, null, 2)}`);

    expect(Array.isArray(toolchains)).toBe(true);
    expect(toolchains.length).toBeGreaterThanOrEqual(1);

    const uploaded = toolchains.find(
      (t: any) => t.name === "rust" && t.version === "1.0.0-s3-test"
    );
    expect(uploaded).toBeDefined();
    expect(uploaded.targets.length).toBe(1);
    expect(uploaded.targets[0].target).toBe("x86_64-unknown-linux-gnu");
  });

  test("channel for S3-stored toolchain appears in channels API", async ({ page }) => {
    // Login to get session
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(500);

    const cookies = await page.context().cookies();
    const session = cookies.find(c => c.name === "kellnr_session_id");
    const cookie = `kellnr_session_id=${session!.value}`;

    // List channels via API
    const channelsUrl = `${baseUrl}/api/v1/toolchains/channels`;
    const response = await fetch(channelsUrl, {
      headers: { "Cookie": cookie },
    });

    expect(response.status).toBe(200);
    const channels = await response.json();
    console.log(`[test] Channels list: ${JSON.stringify(channels, null, 2)}`);

    expect(Array.isArray(channels)).toBe(true);

    const stableChannel = channels.find((c: any) => c.name === "stable");
    expect(stableChannel).toBeDefined();
    expect(stableChannel.version).toBe("1.0.0-s3-test");
  });

  test("toolchain stored in S3 is visible in the UI", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(1000);

    // Navigate to settings page and click toolchains tab
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const toolchainsPage = new ToolchainsPage(page);
    await toolchainsPage.clickToolchainsTab();

    // Wait for the toolchains to load
    await page.waitForTimeout(1000);

    // Verify the toolchain is shown (not empty state)
    const hasToolchains = await toolchainsPage.hasToolchains();
    expect(hasToolchains).toBe(true);

    // Verify count is at least 1
    const count = await toolchainsPage.getToolchainCount();
    expect(count).toBeGreaterThanOrEqual(1);
  });

  test("can upload additional target to same toolchain in S3", async ({ page }) => {
    // Login to get session
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(500);

    const cookies = await page.context().cookies();
    const session = cookies.find(c => c.name === "kellnr_session_id");
    const cookie = `kellnr_session_id=${session!.value}`;

    // Read the test archive
    const archiveData = fs.readFileSync(testArchivePath);

    // Upload same version but different target
    const uploadUrl = `${baseUrl}/api/v1/toolchains?name=rust&version=1.0.0-s3-test&target=aarch64-unknown-linux-gnu&date=2024-01-15`;

    const response = await fetch(uploadUrl, {
      method: "PUT",
      headers: {
        "Content-Type": "application/octet-stream",
        "Cookie": cookie,
      },
      body: archiveData,
    });

    expect(response.status).toBe(200);
    const result = await response.json();
    expect(result.success).toBe(true);

    // Verify both targets are now in the manifest
    const manifestUrl = `${baseUrl}/api/v1/toolchains/dist/channel-rust-stable.toml`;
    const manifestResponse = await fetch(manifestUrl, {
      headers: { "Cookie": cookie },
    });
    const manifest = await manifestResponse.text();
    console.log(`[test] Updated manifest:\n${manifest}`);

    expect(manifest).toContain('x86_64-unknown-linux-gnu');
    expect(manifest).toContain('aarch64-unknown-linux-gnu');
  });

  test("can delete toolchain target from S3 storage", async ({ page }) => {
    // Login to get session
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(500);

    const cookies = await page.context().cookies();
    const session = cookies.find(c => c.name === "kellnr_session_id");
    const cookie = `kellnr_session_id=${session!.value}`;

    // Delete the aarch64 target
    const deleteUrl = `${baseUrl}/api/v1/toolchains/rust/1.0.0-s3-test/targets/aarch64-unknown-linux-gnu`;
    const response = await fetch(deleteUrl, {
      method: "DELETE",
      headers: { "Cookie": cookie },
    });

    expect(response.status).toBe(200);
    const result = await response.json();
    expect(result.success).toBe(true);

    // Verify the target is no longer in the list
    const listUrl = `${baseUrl}/api/v1/toolchains`;
    const listResponse = await fetch(listUrl, {
      headers: { "Cookie": cookie },
    });
    const toolchains = await listResponse.json();

    const toolchain = toolchains.find(
      (t: any) => t.name === "rust" && t.version === "1.0.0-s3-test"
    );
    expect(toolchain).toBeDefined();
    expect(toolchain.targets.length).toBe(1);
    expect(toolchain.targets[0].target).toBe("x86_64-unknown-linux-gnu");
  });
});
