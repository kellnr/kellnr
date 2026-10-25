/**
 * UI tests for crate-related views with actual crate data.
 *
 * These tests first upload test crates and then verify:
 * - Crate listing page with real crates
 * - Individual crate detail page
 * - Crate metadata (About tab)
 * - Dependencies display
 * - Versions tab
 * - Settings tab (admin only)
 * - Landing page statistics
 * - Layout: text sizes, page height, phone gutters, sidebar order, install
 *   snippet wrapping and link colors
 *
 * Performance: All tests share a single local Kellnr instance.
 */

import path from "node:path";
import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { CratesPage, CratePage, LandingPage, LoginPage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  publishCrate,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";

/**
 * Helper to publish test crates to a running Kellnr instance.
 * Uses test-auth-req crates which have no crates.io dependencies.
 */
async function publishTestCrates(log: (msg: string) => void): Promise<void> {
  const registry = "kellnr-test";
  const tokenSourceCrateDir = path.resolve(
    process.cwd(),
    "crates",
    "test-auth-req",
    "foo-bar"
  );
  const registryToken = extractRegistryTokenFromCargoConfig({
    crateDir: tokenSourceCrateDir,
    registryName: registry,
  });

  log("Publishing crate: test_lib");
  await publishCrate({
    cratePath: "tests/crates/test-auth-req/test_lib",
    registry,
    registryToken,
  });

  log("Publishing crate: foo-bar");
  await publishCrate({
    cratePath: "tests/crates/test-auth-req/foo-bar",
    registry,
    registryToken,
  });
}

test.describe("Crate Views with Data", () => {
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
      name: `kellnr-crates-data-${suffix}`,
      env: {
        // Don't require auth for viewing tests - only for admin tests
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

  test("crates page shows uploaded crates", async ({ page }) => {
    await page.goto(`${baseUrl}/crates`);
    const cratesPage = new CratesPage(page);
    await cratesPage.waitForSearchResults();

    // Should not show empty state
    const hasNoCrates = await cratesPage.hasNoCrates();
    expect(hasNoCrates).toBe(false);

    // Should have our published crates
    const hasFooBar = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBar).toBe(true);

    const hasTestLib = await cratesPage.hasCrate("test_lib");
    expect(hasTestLib).toBe(true);

    // Search for specific crate
    await cratesPage.search("foo");
    await cratesPage.waitForSearchResults();

    // Should find foo-bar
    const hasFooBarAfterSearch = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBarAfterSearch).toBe(true);

    // Should not show test_lib (doesn't match "foo")
    const hasTestLibAfterSearch = await cratesPage.hasCrate("test_lib");
    expect(hasTestLibAfterSearch).toBe(false);

    // Clear search shows all crates
    await cratesPage.clearSearch();
    await cratesPage.waitForSearchResults();

    // Should show all crates again
    const hasFooBarAfterClear = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBarAfterClear).toBe(true);
  });

  test("crate detail page displays crate information", async ({ page }) => {
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    // Verify crate header
    const name = await cratePage.getCrateName();
    expect(name).toBe("foo-bar");

    const version = await cratePage.getVersion();
    expect(version).toBe("1.0.0");

    const description = await cratePage.getDescription();
    expect(description).toContain("Test hyphens in crate names");

    // Verify About tab metadata
    await cratePage.clickTab("about");
    await page.waitForTimeout(500);

    // Check for authors (foo-bar has secana)
    const aboutContent = await page.locator(".v-card").filter({ hasText: "Authors" }).textContent();
    expect(aboutContent).toContain("secana");

    // Verify Dependencies tab
    await cratePage.clickTab("dependencies");
    await page.waitForTimeout(500);

    // Check for normal dependencies - foo-bar depends on test_lib
    // Look for the dependency name in the dependencies section
    const testLibDep = page.locator(".dep-name").filter({ hasText: "test_lib" });
    await expect(testLibDep).toBeVisible();

    // Verify Versions tab
    await cratePage.clickTab("versions");
    await page.waitForTimeout(500);

    // Should show version 1.0.0 - use first() to avoid strict mode violation (version appears multiple times)
    const versionText = page.getByText("1.0.0").first();
    await expect(versionText).toBeVisible();

    // Verify sidebar install snippet
    const installSnippet = await cratePage.getInstallSnippet();
    expect(installSnippet).toContain("foo-bar");
    expect(installSnippet).toContain("1.0.0");
  });

  test("navigate from crates list to crate detail", async ({ page }) => {
    await page.goto(`${baseUrl}/crates`);
    const cratesPage = new CratesPage(page);
    await cratesPage.waitForSearchResults();

    // Click on foo-bar crate
    await cratesPage.clickCrate("foo-bar");

    // Wait for navigation
    await page.waitForURL("**/crate?name=foo-bar**");

    // Verify crate detail page loaded
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    const name = await cratePage.getCrateName();
    expect(name).toBe("foo-bar");
  });

  test("landing page shows statistics with crates", async ({ page }) => {
    await page.goto(`${baseUrl}/`);
    const landingPage = new LandingPage(page);
    await landingPage.waitForPageLoad();

    // Verify statistics show crate count
    await landingPage.waitForStatistics();

    // Should show 2 crates (test_lib, foo-bar)
    const crateCount = await landingPage.getTotalCratesCount();
    expect(crateCount).toBe(2);

    // Verify search from landing page
    await landingPage.search("foo");

    // Should navigate to crates page with search
    await page.waitForURL("**/crates?search=foo**");

    const cratesPage = new CratesPage(page);
    await cratesPage.waitForSearchResults();

    // Should find foo-bar
    const hasFooBar = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBar).toBe(true);
  });

  test("admin can see Settings and Admin tabs", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to crate detail page
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    // Verify admin tabs are visible
    // Settings tab should be visible for admin
    const settingsVisible = await cratePage.isTabVisible("settings");
    expect(settingsVisible).toBe(true);

    // Admin tab should be visible for admin
    const adminVisible = await cratePage.isTabVisible("admin");
    expect(adminVisible).toBe(true);

    // Open Settings tab and verify content
    await cratePage.clickTab("settings");
    await page.waitForTimeout(500);

    // Should see crate owners section - new UI uses settings-card
    const ownersCard = page.locator(".settings-card").filter({ hasText: "Crate Owners" });
    await expect(ownersCard).toBeVisible();

    // Should see access control section - new UI uses settings-card
    const accessCard = page.locator(".settings-card").filter({ hasText: "Access Control" });
    await expect(accessCard).toBeVisible();

    // Open Admin tab and verify content
    await cratePage.clickTab("admin");
    await page.waitForTimeout(500);

    // Should see delete buttons
    await expect(cratePage.deleteVersionButton).toBeVisible();
    await expect(cratePage.deleteCrateButton).toBeVisible();

    // Should see warning about deleting - new text mentions "yanking"
    const warning = page.locator(".v-alert").filter({ hasText: "yanking" });
    await expect(warning).toBeVisible();
  });

  test.describe("layout", () => {
    test("landing page statistics use large numbers", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(`${baseUrl}/`);
      const landingPage = new LandingPage(page);
      await landingPage.waitForStatistics();

      // The text size classes are Vuetify 4 names; with the old Vuetify 3
      // names the numbers fell back to the 14px body size.
      const fontSize = await page
        .getByTestId("stat-value")
        .first()
        .evaluate((el) => parseFloat(getComputedStyle(el).fontSize));
      expect(fontSize).toBeGreaterThanOrEqual(28);
    });

    test("short pages do not scroll", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      for (const route of ["/", "/login", "/docqueue", "/crates"]) {
        await page.goto(`${baseUrl}${route}`);
        await page.waitForLoadState("networkidle");
        const { scrollHeight, innerHeight } = await page.evaluate(() => ({
          scrollHeight: document.documentElement.scrollHeight,
          innerHeight: window.innerHeight,
        }));
        expect(scrollHeight, `page ${route} should fit the window`).toBeLessThanOrEqual(innerHeight);
      }
    });

    test("crate cards fit a phone screen", async ({ page }) => {
      await page.setViewportSize({ width: 390, height: 844 });
      await page.goto(`${baseUrl}/crates`);
      const cratesPage = new CratesPage(page);
      await cratesPage.waitForSearchResults();

      const card = page.getByTestId("crate-card").first();
      const cardBox = await card.boundingBox();
      const statsBox = await card.locator(".stats-row").boundingBox();
      expect(cardBox).not.toBeNull();
      expect(statsBox).not.toBeNull();
      // Downloads, date and documentation button stay inside the card
      expect(statsBox!.x + statsBox!.width).toBeLessThanOrEqual(cardBox!.x + cardBox!.width);

      // The date stays on one line
      const lineHeights = await card.getByTestId("crate-card-updated").evaluate((el) => ({
        height: el.getBoundingClientRect().height,
        lineHeight: parseFloat(getComputedStyle(el).lineHeight),
      }));
      expect(lineHeights.height).toBeLessThan(lineHeights.lineHeight * 1.5);

      // Page content has a side gutter and no horizontal scrolling
      expect(cardBox!.x).toBeGreaterThanOrEqual(12);
      const scrollWidth = await page.evaluate(() => document.documentElement.scrollWidth);
      expect(scrollWidth).toBeLessThanOrEqual(390);
    });

    test("landing page has a side gutter on a phone", async ({ page }) => {
      await page.setViewportSize({ width: 390, height: 844 });
      await page.goto(`${baseUrl}/`);
      const landingPage = new LandingPage(page);
      await landingPage.waitForStatistics();

      const cardBox = await page.getByTestId("stat-value").first().locator("xpath=ancestor::*[contains(@class,'v-card')][1]").boundingBox();
      expect(cardBox).not.toBeNull();
      expect(cardBox!.x).toBeGreaterThanOrEqual(12);
      expect(cardBox!.x + cardBox!.width).toBeLessThanOrEqual(390 - 12);
    });

    test("crate page shows content before the sidebar on a phone", async ({ page }) => {
      await page.setViewportSize({ width: 390, height: 844 });
      await page.goto(`${baseUrl}/crate?name=foo-bar`);
      const cratePage = new CratePage(page);
      await cratePage.waitForCrateData();

      const contentBox = await page.locator(".content-card").first().boundingBox();
      const sidebarBox = await cratePage.sidebar.boundingBox();
      expect(contentBox).not.toBeNull();
      expect(sidebarBox).not.toBeNull();
      expect(contentBox!.y).toBeLessThan(sidebarBox!.y);
      expect(sidebarBox!.x).toBeGreaterThanOrEqual(12);
    });

    test("install snippet does not break inside the version", async ({ page }) => {
      // Smallest two-column width, where the sidebar is narrowest and the
      // snippet has to wrap
      await page.setViewportSize({ width: 960, height: 900 });
      await page.goto(`${baseUrl}/crate?name=foo-bar`);
      const cratePage = new CratePage(page);
      await cratePage.waitForCrateData();

      // The snippet only wraps between words, so every word is on one line
      const words = await page.getByTestId("install-snippet").locator(".word").evaluateAll((els) =>
        els.map((el) => ({
          text: el.textContent,
          lines: el.getClientRects().length,
        })),
      );
      expect(words.map((w) => w.text)).toContain('"1.0.0",');
      for (const word of words) {
        expect(word.lines, `"${word.text}" should not wrap`).toBe(1);
      }
    });

    test("documentation link uses the theme color", async ({ page }) => {
      await page.goto(`${baseUrl}/crate?name=foo-bar`);
      const cratePage = new CratePage(page);
      await cratePage.waitForCrateData();

      // Browsers report the unvisited style for :visited links, so check that
      // the link sets the theme color itself instead of the browser default.
      const { color, primary } = await page.getByTestId("sidebar-add-docs").evaluate((el) => ({
        color: getComputedStyle(el).color,
        primary: getComputedStyle(document.querySelector(".v-application")!)
          .getPropertyValue("--v-theme-primary")
          .trim(),
      }));
      expect(color).toBe(`rgb(${primary.split(",").map((c) => c.trim()).join(", ")})`);
    });
  });
});
