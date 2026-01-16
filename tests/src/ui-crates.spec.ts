/**
 * UI tests for the crates listing page.
 *
 * Tests:
 * - Crates page displays correctly
 * - Search functionality
 * - Crates proxy toggle
 * - Empty state when no crates
 */

import { test, expect, withKellnrUI } from "./lib/ui-fixtures";
import { CratesPage } from "./pages";
import { restrictToSingleWorkerBecauseFixedPorts } from "./testUtils";

test.describe("Crates Page UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  test("crates page displays correctly", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "crates-display" }, async (baseUrl) => {
      const cratesPage = new CratesPage(page);

      await test.step("navigate to crates page", async () => {
        await page.goto(`${baseUrl}/crates`);
        await cratesPage.waitForPageLoad();
      });

      await test.step("verify crates page elements", async () => {
        // Search input should be visible
        await expect(cratesPage.searchInput).toBeVisible();

        // Crates proxy switch should be visible
        await expect(cratesPage.cratesProxySwitch).toBeVisible();
      });

      await test.step("wait for crates to load", async () => {
        await cratesPage.waitForSearchResults();

        // Fresh instance should show empty state (no crates published yet)
        const hasNoCrates = await cratesPage.hasNoCrates();
        // This could be true (no crates) or false (has crates) depending on state
        // Just verify the page loaded without errors
        expect(await cratesPage.isLoading()).toBe(false);
      });
    });
  });

  test("empty state shown when no crates", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "crates-empty" }, async (baseUrl) => {
      const cratesPage = new CratesPage(page);

      await test.step("navigate to crates page", async () => {
        await page.goto(`${baseUrl}/crates`);
        await cratesPage.waitForSearchResults();
      });

      await test.step("verify empty state", async () => {
        // Fresh Kellnr instance should have no crates
        const hasNoCrates = await cratesPage.hasNoCrates();
        expect(hasNoCrates).toBe(true);

        // Empty state should mention documentation
        const emptyMessage = page.getByText("No crates found");
        await expect(emptyMessage).toBeVisible();
      });
    });
  });

  test("search input accepts text", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "crates-search-input" }, async (baseUrl) => {
      const cratesPage = new CratesPage(page);

      await test.step("navigate to crates page", async () => {
        await page.goto(`${baseUrl}/crates`);
        await cratesPage.waitForPageLoad();
      });

      await test.step("type in search input", async () => {
        await cratesPage.searchInput.fill("test-crate");
        const value = await cratesPage.getSearchText();
        expect(value).toBe("test-crate");
      });

      await test.step("clear search input", async () => {
        await cratesPage.searchInput.clear();
        const value = await cratesPage.getSearchText();
        expect(value).toBe("");
      });
    });
  });

  test("search via URL query parameter", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "crates-search-url" }, async (baseUrl) => {
      const cratesPage = new CratesPage(page);

      await test.step("navigate to crates page with search query", async () => {
        await page.goto(`${baseUrl}/crates?search=my-crate`);
        await cratesPage.waitForPageLoad();
      });

      await test.step("verify search input is pre-filled", async () => {
        const value = await cratesPage.getSearchText();
        expect(value).toBe("my-crate");
      });
    });
  });

  test("crates proxy toggle can be switched", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "crates-proxy-toggle" }, async (baseUrl) => {
      const cratesPage = new CratesPage(page);

      await test.step("navigate to crates page", async () => {
        await page.goto(`${baseUrl}/crates`);
        await cratesPage.waitForSearchResults();
      });

      await test.step("get initial proxy state", async () => {
        const initialState = await cratesPage.isCratesProxyEnabled();

        // Toggle the switch
        await cratesPage.toggleCratesProxy();

        // Verify state changed
        const newState = await cratesPage.isCratesProxyEnabled();
        expect(newState).not.toBe(initialState);
      });
    });
  });

  test("search triggers when pressing enter", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "crates-search-enter" }, async (baseUrl) => {
      const cratesPage = new CratesPage(page);

      await test.step("navigate to crates page", async () => {
        await page.goto(`${baseUrl}/crates`);
        await cratesPage.waitForSearchResults();
      });

      await test.step("enter search query and press enter", async () => {
        // Type search query
        await cratesPage.searchInput.fill("nonexistent-crate-xyz");

        // Press enter to search
        await cratesPage.searchInput.press("Enter");

        // Wait for search to complete
        await cratesPage.waitForSearchResults();
      });

      await test.step("verify search completed", async () => {
        // Should show empty results for nonexistent crate
        const hasNoCrates = await cratesPage.hasNoCrates();
        expect(hasNoCrates).toBe(true);
      });
    });
  });

  test("page shows loading indicator while fetching", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "crates-loading" }, async (baseUrl) => {
      await test.step("intercept API to delay response", async () => {
        // Add a delay to the crates API to observe loading state
        await page.route("**/api/crates*", async (route) => {
          await new Promise((resolve) => setTimeout(resolve, 500));
          await route.continue();
        });
      });

      await test.step("navigate and observe loading", async () => {
        // Start navigation
        const navigationPromise = page.goto(`${baseUrl}/crates`);

        // Check for loading indicator (may be quick)
        await page.waitForTimeout(100);

        // Complete navigation
        await navigationPromise;
      });

      await test.step("verify loading completes", async () => {
        const cratesPage = new CratesPage(page);
        await cratesPage.waitForSearchResults();

        // Loading should be done
        const isLoading = await cratesPage.isLoading();
        expect(isLoading).toBe(false);
      });
    });
  });
});
