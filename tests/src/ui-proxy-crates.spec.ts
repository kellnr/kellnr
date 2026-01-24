/**
 * UI tests for crates.io proxy functionality.
 *
 * These tests verify:
 * - Crates proxy toggle functionality
 * - Cached crates display when proxy is enabled
 * - Landing page shows cached crates section
 * - Searching for crates.io crates via proxy
 *
 * Performance: All tests share a single local Kellnr instance.
 */

import path from "node:path";
import { test, expect } from "./lib/ui-fixtures";
import { CratesPage, LandingPage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  publishCrate,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";

/**
 * Helper to publish test crates to a running Kellnr instance.
 * Uses test-sparse-registry crates which require the proxy to be enabled.
 * Must publish dependencies (test_lib, UpperCase-Name123) before foo-bar.
 */
async function publishTestCrates(log: (msg: string) => void): Promise<void> {
  const registry = "kellnr-test";
  const tokenSourceCrateDir = path.resolve(
    process.cwd(),
    "crates",
    "test-sparse-registry",
    "foo-bar"
  );
  const registryToken = extractRegistryTokenFromCargoConfig({
    crateDir: tokenSourceCrateDir,
    registryName: registry,
  });

  // Publish dependencies first
  log("Publishing crate: test_lib");
  await publishCrate({
    cratePath: "tests/crates/test-sparse-registry/test_lib",
    registry,
    registryToken,
  });

  log("Publishing crate: UpperCase-Name123");
  await publishCrate({
    cratePath: "tests/crates/test-sparse-registry/UpperCase-Name123",
    registry,
    registryToken,
  });

  // Now publish foo-bar which depends on test_lib and UpperCase-Name123
  log("Publishing crate: foo-bar");
  await publishCrate({
    cratePath: "tests/crates/test-sparse-registry/foo-bar",
    registry,
    registryToken,
  });
}

test.describe("Crates.io Proxy UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    // Setup needs more time for publishing crates
    test.setTimeout(120_000);

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    const suffix = `${Date.now()}`;

    started = await startLocalKellnr({
      name: `kellnr-proxy-${suffix}`,
      env: {
        KELLNR_PROXY__ENABLED: "true",
        // Note: Do NOT set auth_required here - the proxy needs anonymous access
        // for cargo to resolve crates.io dependencies
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl}`);

    console.log("[setup] Publishing test crates");
    await publishTestCrates(console.log);
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
    }
  });

  test("crates proxy toggle switches between own and cached crates", async ({ page }) => {
    await page.goto(`${baseUrl}/crates`);
    const cratesPage = new CratesPage(page);
    await cratesPage.waitForSearchResults();

    // Initially proxy should be off (showing own crates)
    const isProxyEnabled = await cratesPage.isCratesProxyEnabled();
    expect(isProxyEnabled).toBe(false);

    // Should show our published crates
    const hasFooBar = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBar).toBe(true);

    // Toggle proxy on
    await cratesPage.toggleCratesProxy();
    await cratesPage.waitForSearchResults();

    // Proxy should now be enabled
    const isProxyEnabledAfterToggle = await cratesPage.isCratesProxyEnabled();
    expect(isProxyEnabledAfterToggle).toBe(true);

    // With proxy enabled, we should see cached crates.io crates (from dependencies)
    // The publishing process caches crates.io dependencies through the proxy
    // Crates.io crates are marked with "Crates.io logo" in their cards
    const cratesIoCard = page.locator(".crate-card").filter({
      has: page.locator("img[alt='Crates.io logo']"),
    });

    // There should be at least one cached crates.io crate (from dependency resolution)
    const cratesIoCount = await cratesIoCard.count();
    expect(cratesIoCount).toBeGreaterThan(0);

    // Toggle proxy off again
    await cratesPage.toggleCratesProxy();
    await cratesPage.waitForSearchResults();

    // Proxy should be disabled
    const isProxyDisabled = await cratesPage.isCratesProxyEnabled();
    expect(isProxyDisabled).toBe(false);

    // Own crates should be visible again
    const hasFooBarAgain = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBarAgain).toBe(true);
  });

  test("landing page shows cached crates section when proxy enabled", async ({ page }) => {
    await page.goto(`${baseUrl}/`);
    const landingPage = new LandingPage(page);
    await landingPage.waitForPageLoad();

    // When proxy is enabled, the cached crates section should be visible
    const hasCachedSection = await landingPage.hasCachedCratesSection();
    expect(hasCachedSection).toBe(true);
  });

  test("crates page with proxy shows different statistics", async ({ page }) => {
    // Navigate to landing page and check stats
    await page.goto(`${baseUrl}/`);
    const landingPage = new LandingPage(page);
    await landingPage.waitForStatistics();

    // Should show 3 own crates (test_lib, UpperCase-Name123, foo-bar)
    const crateCount = await landingPage.getTotalCratesCount();
    expect(crateCount).toBe(3);

    // Navigate to crates page
    await page.goto(`${baseUrl}/crates`);
    const cratesPage = new CratesPage(page);
    await cratesPage.waitForSearchResults();

    // Proxy switch should be visible when proxy is enabled
    const proxySwitch = page.locator(".v-switch").filter({ hasText: "Crates proxy" });
    await expect(proxySwitch).toBeVisible();

    // Toggle to proxy view and verify crates.io crates
    await cratesPage.toggleCratesProxy();
    await cratesPage.waitForSearchResults();

    // Publishing crates caches their crates.io dependencies through the proxy
    // So we should see cached crates.io crates (identified by Crates.io logo)
    const cratesIoCard = page.locator(".crate-card").filter({
      has: page.locator("img[alt='Crates.io logo']"),
    });
    const cratesIoCount = await cratesIoCard.count();
    expect(cratesIoCount).toBeGreaterThan(0);
  });
});
