/**
 * UI tests for navigation and layout.
 *
 * Tests:
 * - Header navigation links work correctly
 * - Logo navigates to home
 * - Theme toggle works
 * - Landing page displays correctly
 *
 * Performance: All tests share a single Kellnr container instance.
 */

import { test, expect } from "./lib/ui-fixtures";
import { HeaderComponent, LandingPage, CratesPage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  waitForHttpOk,
  assertDockerAvailable,
} from "./testUtils";
import { startKellnr, type StartedKellnr } from "./lib/kellnr";

test.describe("Navigation UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    // Container setup needs more time than default timeout
    test.setTimeout(120_000); // 2 minutes for setup

    await assertDockerAvailable();
    console.log("[setup] Docker is available");

    const image = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";
    const suffix = `${Date.now()}`;

    started = await startKellnr(
      {
        name: `kellnr-navigation-${suffix}`,
        image,
        env: {
          // No special config needed for navigation tests
        },
      },
      { title: "navigation" } as any
    );

    baseUrl = started.baseUrl;

    console.log(`[setup] Waiting for HTTP 200 on ${baseUrl}`);
    await waitForHttpOk(baseUrl, {
      timeoutMs: 60_000,
      intervalMs: 1_000,
    });
    console.log("[setup] Server ready");
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping container");
      await started.started.container.stop();
    }
  });

  test("landing page displays correctly", async ({ page }) => {
    const landingPage = new LandingPage(page);

    await page.goto(baseUrl);
    await landingPage.waitForPageLoad();

    // Should show Kellnr branding
    const hasBranding = await landingPage.hasKellnrBranding();
    expect(hasBranding).toBe(true);

    // Should have search input
    await expect(landingPage.searchInput).toBeVisible();

    // Wait for statistics to load
    await landingPage.waitForStatistics();

    // Statistics should be visible (even if zero)
    const hasStats = await landingPage.hasStatistics();
    expect(hasStats).toBe(true);
  });

  test("header navigation - search link", async ({ page }) => {
    const header = new HeaderComponent(page);

    // Start on landing page
    await page.goto(baseUrl);
    await page.waitForLoadState("networkidle");

    // Click search navigation link
    await header.navigateToSearch();

    // Verify navigation to crates page
    expect(page.url()).toContain("/crates");

    const cratesPage = new CratesPage(page);
    await expect(cratesPage.searchInput).toBeVisible();
  });

  test("header navigation - logo returns home", async ({ page }) => {
    const header = new HeaderComponent(page);
    const landingPage = new LandingPage(page);

    // Navigate to crates page first
    await page.goto(`${baseUrl}/crates`);
    await page.waitForLoadState("networkidle");
    expect(page.url()).toContain("/crates");

    // Click logo to return home
    await header.clickLogo();
    await page.waitForLoadState("networkidle");

    // Verify back on landing page
    // URL might be "/" or "/index.html" depending on deployment
    const url = page.url();
    expect(url).not.toContain("/crates");

    // Also verify landing page branding is visible
    const hasBranding = await landingPage.hasKellnrBranding();
    expect(hasBranding).toBe(true);
  });

  test("header navigation - doc queue link", async ({ page }) => {
    const header = new HeaderComponent(page);

    // Start on landing page
    await page.goto(baseUrl);
    await page.waitForLoadState("networkidle");

    // Click doc queue navigation link
    await header.navigateToDocQueue();

    // Verify navigation to doc queue page
    expect(page.url()).toContain("/docqueue");
  });

  test("theme toggle switches between light and dark mode", async ({ page }) => {
    const header = new HeaderComponent(page);

    // Navigate to landing page
    await page.goto(baseUrl);
    await page.waitForLoadState("networkidle");

    // Record initial state
    const initialDark = await header.isDarkMode();

    // Toggle theme
    await header.toggleTheme();

    // Verify theme changed
    const afterToggle = await header.isDarkMode();
    expect(afterToggle).not.toBe(initialDark);

    // Toggle back
    await header.toggleTheme();

    // Verify back to original
    const afterSecondToggle = await header.isDarkMode();
    expect(afterSecondToggle).toBe(initialDark);
  });

  test("search from landing page navigates to crates", async ({ page }) => {
    const landingPage = new LandingPage(page);

    // Navigate to landing page
    await page.goto(baseUrl);
    await landingPage.waitForPageLoad();

    // Perform search
    await landingPage.search("test-crate");

    // Verify navigation to crates page with search query
    expect(page.url()).toContain("/crates");
    expect(page.url()).toContain("search=test-crate");
  });

  test("login button visible when not authenticated", async ({ page }) => {
    const header = new HeaderComponent(page);

    // Navigate to landing page
    await page.goto(baseUrl);
    await page.waitForLoadState("networkidle");

    // Verify login button is visible
    const isVisible = await header.isLoginButtonVisible();
    expect(isVisible).toBe(true);

    // Click login button navigates to login page
    await header.clickLogin();
    expect(page.url()).toContain("/login");
  });
});
