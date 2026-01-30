/**
 * UI tests for toolchain management functionality.
 *
 * Tests:
 * - Toolchains tab is visible when toolchain feature is enabled
 * - Toolchains tab is not visible when toolchain feature is disabled
 * - Admin can view toolchains section with empty state
 * - Upload form is visible and has correct fields
 * - Upload button is disabled when form is incomplete
 * - API upload, manifest generation, and download work correctly
 * - (Optional) Docker-based rustup integration test
 *
 * Performance: All tests share a single local Kellnr instance per describe block.
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
  startContainer,
  type Started,
} from "./lib/docker";
import fs from "node:fs";
import path from "node:path";

test.describe("Toolchain Management UI Tests - Feature Enabled", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    // Local process setup is faster but still allow extra time
    test.setTimeout(60_000);

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    const suffix = `${Date.now()}`;

    started = await startLocalKellnr({
      name: `kellnr-toolchain-${suffix}`,
      logLevel: "info",
      webLogLevel: "info",
      env: {
        KELLNR_REGISTRY__AUTH_REQUIRED: "true",
        KELLNR_TOOLCHAIN__ENABLED: "true",
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl}`);
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
    }
  });

  test("toolchains tab is visible when feature is enabled", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to settings page
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    // The Toolchains nav item should be visible for admin users when feature is enabled
    const toolchainsPage = new ToolchainsPage(page);
    await expect(toolchainsPage.toolchainsNavItem).toBeVisible();
  });

  test("admin can view toolchains section with empty state", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to settings page and click toolchains tab
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const toolchainsPage = new ToolchainsPage(page);
    await toolchainsPage.clickToolchainsTab();

    // Verify we're on the toolchains section
    await expect(toolchainsPage.sectionHeader).toBeVisible();

    // Verify empty state is shown (no toolchains uploaded yet)
    await expect(toolchainsPage.emptyState).toBeVisible();
  });

  test("upload form has correct fields", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to toolchains section
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const toolchainsPage = new ToolchainsPage(page);
    await toolchainsPage.clickToolchainsTab();

    // Verify all upload form fields are visible
    await expect(toolchainsPage.nameInput).toBeVisible();
    await expect(toolchainsPage.versionInput).toBeVisible();
    await expect(toolchainsPage.targetInput).toBeVisible();
    await expect(toolchainsPage.dateInput).toBeVisible();
    await expect(toolchainsPage.channelSelect).toBeVisible();
    await expect(toolchainsPage.dropZone).toBeVisible();
    await expect(toolchainsPage.uploadButton).toBeVisible();
  });

  test("upload button is disabled when form is incomplete", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to toolchains section
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const toolchainsPage = new ToolchainsPage(page);
    await toolchainsPage.clickToolchainsTab();

    // Verify upload button is disabled when form is empty
    // (name field has default value "rust", but no file is selected)
    const isEnabled = await toolchainsPage.isUploadButtonEnabled();
    expect(isEnabled).toBe(false);
  });

  test("can fill upload form fields", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to toolchains section
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const toolchainsPage = new ToolchainsPage(page);
    await toolchainsPage.clickToolchainsTab();

    // Fill in the form fields
    await toolchainsPage.fillUploadForm({
      name: "rust",
      version: "1.75.0",
      target: "x86_64-unknown-linux-gnu",
      date: "2024-01-15",
    });

    // Verify fields have the expected values
    await expect(toolchainsPage.nameInput).toHaveValue("rust");
    await expect(toolchainsPage.versionInput).toHaveValue("1.75.0");
    await expect(toolchainsPage.targetInput).toHaveValue("x86_64-unknown-linux-gnu");
    await expect(toolchainsPage.dateInput).toHaveValue("2024-01-15");

    // Button should still be disabled because no file is selected
    const isEnabled = await toolchainsPage.isUploadButtonEnabled();
    expect(isEnabled).toBe(false);
  });

  test("non-admin user cannot access toolchains management", async ({ page }) => {
    // First create a non-admin user via admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to user management and create a test user
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const userMgmtNavItem = page.locator(".v-list-item-title").filter({ hasText: "User Management" });
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Fill in the user creation form
    const usernameField = page.locator(".form-field").filter({ hasText: "Username" }).locator("input");
    const passwordField = page.locator(".form-field").filter({ hasText: /^Password$/ }).locator("input");
    const confirmField = page.locator(".form-field").filter({ hasText: "Confirm Password" }).locator("input");

    await usernameField.fill("toolchainuser");
    await passwordField.fill("testpassword");
    await confirmField.fill("testpassword");

    const createButton = page.getByRole("button", { name: "Create User" });
    await createButton.click();

    // Wait for user to be created
    await page.waitForTimeout(1000);

    // Logout admin
    await page.goto(`${baseUrl}`);
    await page.waitForTimeout(500);
    const logoutButton = page.getByRole("button", { name: "Logout" });
    if (await logoutButton.isVisible()) {
      await logoutButton.click();
      await page.waitForTimeout(500);
    }

    // Login as the non-admin user
    await page.goto(`${baseUrl}/login`);
    await loginPage.login("toolchainuser", "testpassword");
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to settings page
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    // Toolchains nav item should not be visible for non-admin users
    const toolchainsPage = new ToolchainsPage(page);
    await expect(toolchainsPage.toolchainsNavItem).not.toBeVisible();
  });
});

test.describe("Toolchain Management UI Tests - Feature Disabled", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    test.setTimeout(60_000);

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available (feature disabled test)");

    const suffix = `${Date.now()}`;

    // Start Kellnr with toolchain DISABLED
    started = await startLocalKellnr({
      name: `kellnr-toolchain-disabled-${suffix}`,
      logLevel: "info",
      webLogLevel: "info",
      env: {
        KELLNR_REGISTRY__AUTH_REQUIRED: "true",
        KELLNR_TOOLCHAIN__ENABLED: "false",
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl}`);
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
    }
  });

  test("toolchains tab is not visible when feature is disabled", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to settings page
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    // The Toolchains nav item should NOT be visible when feature is disabled
    const toolchainsPage = new ToolchainsPage(page);
    await expect(toolchainsPage.toolchainsNavItem).not.toBeVisible();
  });
});

test.describe("Toolchain API and Distribution Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;
  let sessionCookie: string;

  // Path to test archive
  const testArchivePath = path.resolve(
    process.cwd(),
    "fixtures",
    "test-toolchain",
    "rust-1.0.0-test-x86_64-unknown-linux-gnu.tar.xz"
  );

  test.beforeAll(async () => {
    test.setTimeout(120_000);

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    // Verify test archive exists
    if (!fs.existsSync(testArchivePath)) {
      throw new Error(`Test archive not found at ${testArchivePath}. Run create-test-archive.sh first.`);
    }
    console.log(`[setup] Test archive found at ${testArchivePath}`);

    const suffix = `${Date.now()}`;

    started = await startLocalKellnr({
      name: `kellnr-toolchain-api-${suffix}`,
      logLevel: "info",
      webLogLevel: "info",
      env: {
        KELLNR_REGISTRY__AUTH_REQUIRED: "true", // Auth needed for admin APIs
        KELLNR_TOOLCHAIN__ENABLED: "true",
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl}`);
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
    }
  });

  test("can upload toolchain via API", async ({ page }) => {
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
    const uploadUrl = `${baseUrl}/api/v1/toolchain/toolchains?name=rust&version=1.0.0-test&target=x86_64-unknown-linux-gnu&date=2024-01-15&channel=stable`;

    const response = await fetch(uploadUrl, {
      method: "PUT",
      headers: {
        "Content-Type": "application/octet-stream",
        "Cookie": sessionCookie,
      },
      body: archiveData,
    });

    expect(response.status).toBe(200);
    const result = await response.json();
    expect(result.success).toBe(true);
    console.log(`[test] Upload response: ${JSON.stringify(result)}`);
  });

  test("can fetch channel manifest", async () => {
    // The manifest should be available after upload (requires auth when auth_required is true)
    const manifestUrl = `${baseUrl}/api/v1/toolchain/dist/channel-rust-stable.toml`;

    const response = await fetch(manifestUrl, {
      headers: { "Cookie": sessionCookie },
    });
    expect(response.status).toBe(200);

    const manifest = await response.text();
    console.log(`[test] Manifest content:\n${manifest}`);

    // Verify manifest structure
    expect(manifest).toContain('manifest-version = "2"');
    expect(manifest).toContain('[pkg.rust]');
    expect(manifest).toContain('version = "1.0.0-test"');
    expect(manifest).toContain('x86_64-unknown-linux-gnu');
    expect(manifest).toContain('available = true');
    expect(manifest).toContain('url = "');
    expect(manifest).toContain('hash = "');
  });

  test("can download toolchain archive", async () => {
    // First get the manifest to find the archive URL
    const manifestUrl = `${baseUrl}/api/v1/toolchain/dist/channel-rust-stable.toml`;
    const manifestResponse = await fetch(manifestUrl, {
      headers: { "Cookie": sessionCookie },
    });
    const manifest = await manifestResponse.text();

    // Extract URL from manifest
    const urlMatch = manifest.match(/url = "([^"]+)"/);
    expect(urlMatch).toBeDefined();
    const archiveUrl = urlMatch![1];
    console.log(`[test] Archive URL: ${archiveUrl}`);

    // Download the archive (also requires auth)
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

  test("toolchain appears in list API", async ({ page }) => {
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
    const listUrl = `${baseUrl}/api/v1/toolchain/toolchains`;
    const response = await fetch(listUrl, {
      headers: { "Cookie": cookie },
    });

    expect(response.status).toBe(200);
    const toolchains = await response.json();
    console.log(`[test] Toolchains list: ${JSON.stringify(toolchains, null, 2)}`);

    expect(Array.isArray(toolchains)).toBe(true);
    expect(toolchains.length).toBeGreaterThanOrEqual(1);

    const uploaded = toolchains.find(
      (t: any) => t.name === "rust" && t.version === "1.0.0-test"
    );
    expect(uploaded).toBeDefined();
    expect(uploaded.targets.length).toBe(1);
    expect(uploaded.targets[0].target).toBe("x86_64-unknown-linux-gnu");
  });

  test("channel appears in channels API", async ({ page }) => {
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
    const channelsUrl = `${baseUrl}/api/v1/toolchain/channels`;
    const response = await fetch(channelsUrl, {
      headers: { "Cookie": cookie },
    });

    expect(response.status).toBe(200);
    const channels = await response.json();
    console.log(`[test] Channels list: ${JSON.stringify(channels, null, 2)}`);

    expect(Array.isArray(channels)).toBe(true);

    const stableChannel = channels.find((c: any) => c.name === "stable");
    expect(stableChannel).toBeDefined();
    expect(stableChannel.version).toBe("1.0.0-test");
  });

  test("toolchain appears in UI list", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(500);

    // Navigate to toolchains section
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const toolchainsPage = new ToolchainsPage(page);
    await toolchainsPage.clickToolchainsTab();

    // Verify toolchain is visible
    const count = await toolchainsPage.getToolchainCount();
    expect(count).toBeGreaterThanOrEqual(1);

    // Check for the uploaded toolchain
    const hasToolchains = await toolchainsPage.hasToolchains();
    expect(hasToolchains).toBe(true);
  });

  test("can view toolchain channel in UI", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(500);

    // Navigate to toolchains section
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const toolchainsPage = new ToolchainsPage(page);
    await toolchainsPage.clickToolchainsTab();

    // Check that the toolchain shows its channel
    const channel = await toolchainsPage.getToolchainChannel("rust", "1.0.0-test");
    expect(channel).toBe("stable");
  });
});

test.describe("Toolchain UI Channel and Delete Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;
  let sessionCookie: string;

  // Path to test archive
  const testArchivePath = path.resolve(
    process.cwd(),
    "fixtures",
    "test-toolchain",
    "rust-1.0.0-test-x86_64-unknown-linux-gnu.tar.xz"
  );

  test.beforeAll(async () => {
    test.setTimeout(120_000);

    assertKellnrBinaryExists();

    // Verify test archive exists
    if (!fs.existsSync(testArchivePath)) {
      throw new Error(`Test archive not found at ${testArchivePath}. Run create-test-archive.sh first.`);
    }

    const suffix = `${Date.now()}`;

    started = await startLocalKellnr({
      name: `kellnr-toolchain-ui-${suffix}`,
      logLevel: "info",
      webLogLevel: "info",
      env: {
        KELLNR_REGISTRY__AUTH_REQUIRED: "true",
        KELLNR_TOOLCHAIN__ENABLED: "true",
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl}`);
  });

  test.afterAll(async () => {
    if (started) {
      await started.stop();
    }
  });

  async function uploadToolchain(
    name: string,
    version: string,
    target: string,
    date: string,
    channel?: string
  ): Promise<void> {
    const archiveData = fs.readFileSync(testArchivePath);
    let uploadUrl = `${baseUrl}/api/v1/toolchain/toolchains?name=${name}&version=${version}&target=${target}&date=${date}`;
    if (channel) {
      uploadUrl += `&channel=${channel}`;
    }

    const response = await fetch(uploadUrl, {
      method: "PUT",
      headers: {
        "Content-Type": "application/octet-stream",
        "Cookie": sessionCookie,
      },
      body: archiveData,
    });

    if (!response.ok) {
      throw new Error(`Failed to upload toolchain: ${await response.text()}`);
    }
  }

  test("can change toolchain channel via UI dropdown", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(500);

    // Get session cookie for API calls
    const cookies = await page.context().cookies();
    const session = cookies.find(c => c.name === "kellnr_session_id");
    sessionCookie = `kellnr_session_id=${session!.value}`;

    // Upload a toolchain without channel
    await uploadToolchain("rust", "1.80.0", "x86_64-unknown-linux-gnu", "2024-07-01");

    // Navigate to toolchains section
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const toolchainsPage = new ToolchainsPage(page);
    await toolchainsPage.clickToolchainsTab();

    // Verify toolchain has no channel initially
    let channel = await toolchainsPage.getToolchainChannel("rust", "1.80.0");
    expect(channel).toBeNull();

    // Change channel to "beta" via UI
    await toolchainsPage.setToolchainChannel("rust", "1.80.0", "beta");

    // Wait for snackbar confirmation
    const snackbarText = await toolchainsPage.waitForSnackbarAndGetText();
    expect(snackbarText).toContain("beta");
    await toolchainsPage.dismissSnackbar();

    // Reload page to verify persistence
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);
    await toolchainsPage.clickToolchainsTab();

    // Verify channel is now "beta"
    channel = await toolchainsPage.getToolchainChannel("rust", "1.80.0");
    expect(channel).toBe("beta");
  });

  test("can delete entire toolchain via UI", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(500);

    // Get session cookie
    const cookies = await page.context().cookies();
    const session = cookies.find(c => c.name === "kellnr_session_id");
    sessionCookie = `kellnr_session_id=${session!.value}`;

    // Upload a toolchain with multiple targets
    await uploadToolchain("rust", "1.81.0", "x86_64-unknown-linux-gnu", "2024-08-01", "nightly");
    await uploadToolchain("rust", "1.81.0", "aarch64-unknown-linux-gnu", "2024-08-01");

    // Navigate to toolchains section
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const toolchainsPage = new ToolchainsPage(page);
    await toolchainsPage.clickToolchainsTab();

    // Verify toolchain exists with 2 targets
    const targets = await toolchainsPage.getTargetsForToolchain("rust", "1.81.0");
    expect(targets.length).toBe(2);

    // Get initial count
    const initialCount = await toolchainsPage.getToolchainCount();

    // Delete entire toolchain
    await toolchainsPage.deleteToolchain("rust", "1.81.0");

    // Wait for snackbar confirmation
    const snackbarText = await toolchainsPage.waitForSnackbarAndGetText();
    expect(snackbarText).toContain("deleted");
    await toolchainsPage.dismissSnackbar();

    // Verify toolchain count decreased
    const newCount = await toolchainsPage.getToolchainCount();
    expect(newCount).toBe(initialCount - 1);
  });

  test("deleting last target auto-deletes toolchain", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(500);

    // Get session cookie
    const cookies = await page.context().cookies();
    const session = cookies.find(c => c.name === "kellnr_session_id");
    sessionCookie = `kellnr_session_id=${session!.value}`;

    // Upload a toolchain with single target
    await uploadToolchain("rust", "1.82.0", "x86_64-unknown-linux-gnu", "2024-09-01");

    // Navigate to toolchains section
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const toolchainsPage = new ToolchainsPage(page);
    await toolchainsPage.clickToolchainsTab();

    // Verify toolchain exists
    const initialCount = await toolchainsPage.getToolchainCount();
    expect(initialCount).toBeGreaterThanOrEqual(1);

    // Delete the only target
    await toolchainsPage.deleteTarget("rust", "1.82.0", "x86_64-unknown-linux-gnu");

    // Wait for snackbar
    await toolchainsPage.waitForSnackbarAndGetText();
    await toolchainsPage.dismissSnackbar();

    // Verify toolchain was auto-deleted (count decreased)
    const newCount = await toolchainsPage.getToolchainCount();
    expect(newCount).toBe(initialCount - 1);

    // Verify via API that toolchain is gone
    const listUrl = `${baseUrl}/api/v1/toolchain/toolchains`;
    const response = await fetch(listUrl, {
      headers: { "Cookie": sessionCookie },
    });
    const toolchains = await response.json();
    const deleted = toolchains.find(
      (t: any) => t.name === "rust" && t.version === "1.82.0"
    );
    expect(deleted).toBeUndefined();
  });
});

test.describe("Toolchain Docker Integration Test", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;
  let rustupContainer: Started;

  // Path to test archive
  const testArchivePath = path.resolve(
    process.cwd(),
    "fixtures",
    "test-toolchain",
    "rust-1.0.0-test-x86_64-unknown-linux-gnu.tar.xz"
  );

  const testScriptPath = path.resolve(process.cwd(), "fixtures", "test-toolchain", "test-rustup.sh");

  test.beforeAll(async () => {
    // Docker tests need more time
    test.setTimeout(10 * 60 * 1000); // 10 minutes

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    await assertDockerAvailable();
    console.log("[setup] Docker is available");

    // Verify test archive exists
    if (!fs.existsSync(testArchivePath)) {
      throw new Error(`Test archive not found at ${testArchivePath}. Run create-test-archive.sh first.`);
    }

    // Verify test script exists
    if (!fs.existsSync(testScriptPath)) {
      throw new Error(`Test script not found at ${testScriptPath}`);
    }

    const suffix = `${Date.now()}`;

    // Start Kellnr
    // Note: auth_required is false for Docker test since rustup doesn't support auth
    // The toolchain dist server should be public for rustup to work
    started = await startLocalKellnr({
      name: `kellnr-toolchain-docker-${suffix}`,
      logLevel: "info",
      webLogLevel: "info",
      env: {
        KELLNR_REGISTRY__AUTH_REQUIRED: "false",
        KELLNR_TOOLCHAIN__ENABLED: "true",
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Kellnr ready at ${baseUrl}`);

    // Login to get session cookie
    console.log("[setup] Logging in...");
    const loginResponse = await fetch(`${baseUrl}/api/v1/user/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        user: DEFAULT_ADMIN_USER,
        pwd: DEFAULT_ADMIN_PASSWORD,
      }),
    });

    if (!loginResponse.ok) {
      throw new Error(`Login failed: ${await loginResponse.text()}`);
    }

    // Extract session cookie from Set-Cookie header
    const setCookie = loginResponse.headers.get("set-cookie");
    if (!setCookie) {
      throw new Error("No session cookie returned from login");
    }
    const sessionCookieMatch = setCookie.match(/kellnr_session_id=([^;]+)/);
    if (!sessionCookieMatch) {
      throw new Error("Could not extract kellnr_session_id from cookies");
    }
    const sessionCookie = `kellnr_session_id=${sessionCookieMatch[1]}`;
    console.log("[setup] Login successful");

    // Upload the test toolchain
    console.log("[setup] Uploading test toolchain...");
    const archiveData = fs.readFileSync(testArchivePath);
    const uploadUrl = `${baseUrl}/api/v1/toolchain/toolchains?name=rust&version=1.0.0-test&target=x86_64-unknown-linux-gnu&date=2024-01-15&channel=stable`;

    const uploadResponse = await fetch(uploadUrl, {
      method: "PUT",
      headers: {
        "Content-Type": "application/octet-stream",
        "Cookie": sessionCookie,
      },
      body: archiveData,
    });

    if (!uploadResponse.ok) {
      throw new Error(`Failed to upload toolchain: ${await uploadResponse.text()}`);
    }
    console.log("[setup] Toolchain uploaded successfully");
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    console.log("[teardown] Starting cleanup");

    // Stop rustup container if running
    if (rustupContainer) {
      try {
        console.log("[teardown] Stopping rustup container");
        rustupContainer.stopLogStreaming?.();
        await rustupContainer.container.stop();
      } catch (e) {
        console.log("[teardown] Error stopping rustup container:", e);
      }
    }

    // Stop Kellnr
    if (started) {
      try {
        console.log("[teardown] Stopping Kellnr");
        await started.stop();
      } catch (e) {
        console.log("[teardown] Error stopping Kellnr:", e);
      }
    }

    console.log("[teardown] Cleanup complete");
  });

  test("rustup can connect to Kellnr and parse manifest", async ({}, testInfo) => {
    console.log("[test] Starting rustup container with official rust:slim-trixie image...");

    // Use host.docker.internal on macOS/Windows, or the host IP on Linux
    const kellnrUrl = process.platform === "linux"
      ? `http://172.17.0.1:8000/api/v1/toolchain` // Docker bridge IP on Linux
      : `http://host.docker.internal:8000/api/v1/toolchain`; // macOS/Windows

    // Use the official rust:slim-trixie image which has rustup pre-installed
    // Install curl first (not in slim image), then run the test script
    rustupContainer = await startContainer(
      {
        name: `rustup-test-${Date.now()}`,
        image: "rust:slim-trixie",
        env: {
          KELLNR_DIST_URL: kellnrUrl,
          CHANNEL: "stable",
          VERBOSE: "1",
        },
        bindMounts: {
          [testScriptPath]: "/test-rustup.sh",
        },
        cmd: [
          "bash", "-c",
          "apt-get update && apt-get install -y curl && bash /test-rustup.sh"
        ],
        // Don't wait for ports - this is a one-shot test script
        waitFor: undefined,
      },
      testInfo,
    );

    console.log("[test] Waiting for rustup container to complete...");

    // Wait for container to exit (test completion)
    // Use docker inspect to check container state and get exit code
    const { execa } = await import("execa");
    let exitCode: number | null = null;
    let attempts = 0;
    const maxAttempts = 180; // 3 minutes (pulling image may take time)

    while (attempts < maxAttempts) {
      await new Promise(resolve => setTimeout(resolve, 1000));

      try {
        // Check if container is still running
        const stateResult = await execa("docker", [
          "inspect", "-f", "{{.State.Running}}", rustupContainer.name
        ]);
        const isRunning = stateResult.stdout.trim() === "true";

        if (!isRunning) {
          // Container has stopped - get exit code
          const exitResult = await execa("docker", [
            "inspect", "-f", "{{.State.ExitCode}}", rustupContainer.name
          ]);
          exitCode = parseInt(exitResult.stdout.trim(), 10);
          break;
        }
      } catch (e) {
        // Container might have been removed or other error
        console.log("[test] Container check error:", e);
        break;
      }
      attempts++;
    }

    console.log(`[test] Container exited with code: ${exitCode}`);

    // The test script should exit with 0 if it could connect and parse the manifest
    // It may show warnings about installation failing (expected with minimal archive)
    // but the important thing is that the distribution mechanism works
    expect(exitCode).toBe(0);
  });
});
