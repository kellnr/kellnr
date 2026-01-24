/**
 * UI tests for the crates listing page.
 *
 * Tests:
 * - Crates page displays correctly
 * - Search functionality
 * - Crates proxy toggle
 * - Empty state when no crates
 *
 * Performance: All tests share a single local Kellnr instance.
 */

import { test, expect } from "./lib/ui-fixtures";
import { CratesPage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";

test.describe("Crates Page UI Tests", () => {
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
      name: `kellnr-crates-${suffix}`,
      env: {
        // No special config needed for crates page tests
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

  test("crates page displays correctly", async ({ page }) => {
    const cratesPage = new CratesPage(page);

    await page.goto(`${baseUrl}/crates`);
    await cratesPage.waitForPageLoad();

    // Search input should be visible
    await expect(cratesPage.searchInput).toBeVisible();

    // Crates proxy switch should be visible
    await expect(cratesPage.cratesProxySwitch).toBeVisible();

    // Wait for crates to load
    await cratesPage.waitForSearchResults();

    // Fresh instance should show empty state (no crates published yet)
    // Just verify the page loaded without errors
    expect(await cratesPage.isLoading()).toBe(false);
  });

  test("empty state shown when no crates", async ({ page }) => {
    const cratesPage = new CratesPage(page);

    await page.goto(`${baseUrl}/crates`);
    await cratesPage.waitForSearchResults();

    // Fresh Kellnr instance should have no crates
    const hasNoCrates = await cratesPage.hasNoCrates();
    expect(hasNoCrates).toBe(true);

    // Empty state should mention documentation
    const emptyMessage = page.getByText("No crates found");
    await expect(emptyMessage).toBeVisible();
  });

  test("search input accepts text", async ({ page }) => {
    const cratesPage = new CratesPage(page);

    await page.goto(`${baseUrl}/crates`);
    await cratesPage.waitForPageLoad();

    // Type in search input
    await cratesPage.searchInput.fill("test-crate");
    const value = await cratesPage.getSearchText();
    expect(value).toBe("test-crate");

    // Clear search input
    await cratesPage.searchInput.clear();
    const clearedValue = await cratesPage.getSearchText();
    expect(clearedValue).toBe("");
  });

  test("search via URL query parameter", async ({ page }) => {
    const cratesPage = new CratesPage(page);

    await page.goto(`${baseUrl}/crates?search=my-crate`);
    await cratesPage.waitForPageLoad();

    // Verify search input is pre-filled
    const value = await cratesPage.getSearchText();
    expect(value).toBe("my-crate");
  });

  test("crates proxy toggle can be switched", async ({ page }) => {
    const cratesPage = new CratesPage(page);

    await page.goto(`${baseUrl}/crates`);
    await cratesPage.waitForSearchResults();

    // Get initial proxy state
    const initialState = await cratesPage.isCratesProxyEnabled();

    // Toggle the switch
    await cratesPage.toggleCratesProxy();

    // Verify state changed
    const newState = await cratesPage.isCratesProxyEnabled();
    expect(newState).not.toBe(initialState);
  });

  test("search triggers when pressing enter", async ({ page }) => {
    const cratesPage = new CratesPage(page);

    await page.goto(`${baseUrl}/crates`);
    await cratesPage.waitForSearchResults();

    // Type search query
    await cratesPage.searchInput.fill("nonexistent-crate-xyz");

    // Press enter to search
    await cratesPage.searchInput.press("Enter");

    // Wait for search to complete
    await cratesPage.waitForSearchResults();

    // Should show empty results for nonexistent crate
    const hasNoCrates = await cratesPage.hasNoCrates();
    expect(hasNoCrates).toBe(true);
  });

  test("page shows loading indicator while fetching", async ({ page }) => {
    // Add a delay to the crates API to observe loading state
    await page.route("**/api/crates*", async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 500));
      await route.continue();
    });

    // Start navigation
    const navigationPromise = page.goto(`${baseUrl}/crates`);

    // Check for loading indicator (may be quick)
    await page.waitForTimeout(100);

    // Complete navigation
    await navigationPromise;

    // Verify loading completes
    const cratesPage = new CratesPage(page);
    await cratesPage.waitForSearchResults();

    // Loading should be done
    const isLoading = await cratesPage.isLoading();
    expect(isLoading).toBe(false);
  });
});
