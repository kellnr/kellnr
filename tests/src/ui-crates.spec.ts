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

    // This instance runs without the crates.io proxy, so there is nothing to switch
    await expect(page.getByTestId("crates-proxy-toggle")).toHaveCount(0);

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
    let releaseResponse!: () => void;
    let requestStarted!: () => void;
    const responseGate = new Promise<void>((resolve) => { releaseResponse = resolve; });
    const requestIntercepted = new Promise<void>((resolve) => { requestStarted = resolve; });
    await page.route("**/api/v1/ui/crates*", async (route) => {
      requestStarted();
      await responseGate;
      await route.continue();
    });

    await page.goto(`${baseUrl}/crates`, { waitUntil: "domcontentloaded" });
    const cratesPage = new CratesPage(page);
    try {
      await requestIntercepted;
      await expect(cratesPage.loadingIndicator).toBeVisible();
    } finally {
      releaseResponse();
    }
    await cratesPage.waitForSearchResults();

    await expect(cratesPage.emptyState).toBeVisible();
    await expect(cratesPage.loadingIndicator).toBeHidden();
  });
});
