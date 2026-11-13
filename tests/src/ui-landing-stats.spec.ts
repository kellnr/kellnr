/**
 * UI tests for landing page statistics cards clickability.
 *
 * Tests:
 * - Clicking "Total Crates" card navigates to crates page
 * - Clicking "Cached Crates" card navigates to crates page with proxy enabled
 * - The stat card links carry the proxy mode in the URL
 * - Clicking a crate in the "Recently updated" list navigates to the crate page
 * - Non-clickable cards don't have cursor pointer styling
 *
 * Performance: All tests share a single local Kellnr instance.
 */

import { test, expect } from "./lib/ui-fixtures";
import { LandingPage, CratesPage, CratePage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  assertKellnrBinaryExists,
  publishCrate,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";
import path from "node:path";

test.describe("Landing Page Statistics Cards UI Tests", () => {
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
      name: `kellnr-landing-stats-${suffix}`,
      env: {
        // Enable proxy for cached crates tests
        KELLNR_PROXY__ENABLED: "true",
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl}`);

    // Publish a test crate to ensure we have data
    const registry = "kellnr-test";
    const testCrateDir = path.resolve(
      process.cwd(),
      "crates",
      "test-docs",
      "full-toml"
    );

    const registryToken = extractRegistryTokenFromCargoConfig({
      crateDir: testCrateDir,
      registryName: registry,
    });

    console.log("[setup] Publishing test crate: full-toml");
    await publishCrate({
      cratePath: "tests/crates/test-docs/full-toml",
      registry,
      registryToken,
    });

    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
    }
  });

  test("clicking Total Crates card navigates to crates page", async ({ page }) => {
    const landingPage = new LandingPage(page);
    const cratesPage = new CratesPage(page);

    // Navigate to landing page
    await page.goto(baseUrl);
    await landingPage.waitForStatistics();

    // Verify Total Crates card has pointer cursor
    await expect(landingPage.totalCratesCard).toHaveCSS("cursor", "pointer");

    // Click Total Crates card
    await landingPage.totalCratesCard.click();

    // Should navigate to crates page
    await page.waitForURL(`${baseUrl}/crates`);
    await cratesPage.waitForSearchResults();

    // Verify we're on the crates page
    expect(page.url()).toBe(`${baseUrl}/crates`);

    // Without proxy=1 the list shows the registry crates
    const isProxyEnabled = await cratesPage.isCratesProxyEnabled();
    expect(isProxyEnabled).toBe(false);
  });

  test("clicking Cached Crates card navigates to crates page with proxy enabled", async ({
    page,
  }) => {
    const landingPage = new LandingPage(page);
    const cratesPage = new CratesPage(page);

    // Navigate to landing page
    await page.goto(baseUrl);
    await landingPage.waitForStatistics();

    // Verify Cached Crates section exists (proxy is enabled)
    const hasCachedSection = await landingPage.hasCachedCratesSection();
    expect(hasCachedSection).toBe(true);

    const cachedCratesCard = landingPage.cachedCratesCard;
    await expect(cachedCratesCard).toBeVisible();

    // Verify it has pointer cursor
    await expect(cachedCratesCard).toHaveCSS("cursor", "pointer");

    // Click Cached Crates card
    await cachedCratesCard.click();

    // The proxy mode is part of the URL
    await page.waitForURL(`${baseUrl}/crates?proxy=1`);
    await cratesPage.waitForSearchResults();

    expect(page.url()).toBe(`${baseUrl}/crates?proxy=1`);

    // proxy=1 lists the cached crates.io crates
    const isProxyEnabled = await cratesPage.isCratesProxyEnabled();
    expect(isProxyEnabled).toBe(true);
  });

  test("stat card links carry the proxy mode", async ({ page }) => {
    const landingPage = new LandingPage(page);

    await page.goto(baseUrl);
    await landingPage.waitForStatistics();

    // Plain links, so a new tab opens the same list as a click
    await expect(landingPage.totalCratesCard).toHaveAttribute("href", /\/crates$/);
    await expect(landingPage.totalVersionsCard).toHaveAttribute("href", /\/crates\?sort=updated$/);
    await expect(landingPage.totalDownloadsCard).toHaveAttribute("href", /\/crates\?sort=downloads$/);

    await expect(landingPage.cachedCratesCard).toHaveAttribute("href", /\/crates\?proxy=1$/);
    await expect(landingPage.cachedVersionsCard).toHaveAttribute(
      "href",
      /\/crates\?proxy=1&sort=updated$/
    );
    await expect(landingPage.cachedDownloadsCard).toHaveAttribute(
      "href",
      /\/crates\?proxy=1&sort=downloads$/
    );
  });

  test("clicking a recently updated crate navigates to that crate's page", async ({
    page,
  }) => {
    const landingPage = new LandingPage(page);
    const cratePage = new CratePage(page);

    // Navigate to landing page
    await page.goto(baseUrl);
    await landingPage.waitForStatistics();

    // The first crate in the "Recently updated" list
    const lastUpdatedCard = page.getByTestId("recent-crate").first();
    await expect(lastUpdatedCard).toBeVisible();

    // Verify it has pointer cursor
    await expect(lastUpdatedCard).toHaveCSS("cursor", "pointer");

    // Get the crate name from the card (it's in the .crate-name element)
    const crateName = await lastUpdatedCard
      .locator(".crate-name")
      .textContent();
    expect(crateName).toBeTruthy();

    // Click the Last Updated card
    await lastUpdatedCard.click();

    // Should navigate to the crate page with the crate name
    await page.waitForURL(new RegExp(`${baseUrl}/crate\\?name=`));
    await cratePage.waitForCrateData();

    // Verify we're on the correct crate page
    expect(page.url()).toContain(`/crate?name=${crateName}`);

    // Verify the crate page shows the correct crate
    const displayedCrateName = await cratePage.getCrateName();
    expect(displayedCrateName).toBe(crateName);
  });

  test("clickable cards have pointer cursor", async ({ page }) => {
    const landingPage = new LandingPage(page);

    // Navigate to landing page
    await page.goto(baseUrl);
    await landingPage.waitForStatistics();

    // Total Crates card SHOULD have pointer cursor (it's clickable)
    await expect(landingPage.totalCratesCard).toHaveCSS("cursor", "pointer");

    // Cached Crates card SHOULD have pointer cursor (it's clickable)
    const cachedCratesCard = landingPage.cachedCratesCard;
    if (await cachedCratesCard.isVisible()) {
      await expect(cachedCratesCard).toHaveCSS("cursor", "pointer");
    }
  });

  test("cards have hover effects", async ({ page }) => {
    const landingPage = new LandingPage(page);

    // Navigate to landing page
    await page.goto(baseUrl);
    await landingPage.waitForStatistics();

    // Hover over Total Crates card
    await landingPage.totalCratesCard.hover();

    // Give a moment for the hover animation
    await page.waitForTimeout(500);

    // Card should have transform applied (moved up)
    const transform = await landingPage.totalCratesCard.evaluate((el) => {
      return window.getComputedStyle(el).transform;
    });

    // Transform should be something like "matrix(1, 0, 0, 1, 0, -3)"
    // which represents translateY(-3px)
    expect(transform).not.toBe("none");
  });
});
