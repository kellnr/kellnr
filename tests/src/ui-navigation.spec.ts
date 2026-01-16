/**
 * UI tests for navigation and layout.
 *
 * Tests:
 * - Header navigation links work correctly
 * - Logo navigates to home
 * - Theme toggle works
 * - Landing page displays correctly
 */

import { test, expect, withKellnrUI } from "./lib/ui-fixtures";
import { HeaderComponent, LandingPage, CratesPage } from "./pages";
import { restrictToSingleWorkerBecauseFixedPorts } from "./testUtils";

test.describe("Navigation UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  test("landing page displays correctly", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "landing" }, async (baseUrl) => {
      const landingPage = new LandingPage(page);

      await test.step("navigate to landing page", async () => {
        await page.goto(baseUrl);
        await landingPage.waitForPageLoad();
      });

      await test.step("verify landing page elements", async () => {
        // Should show Kellnr branding
        const hasBranding = await landingPage.hasKellnrBranding();
        expect(hasBranding).toBe(true);

        // Should have search input
        await expect(landingPage.searchInput).toBeVisible();
      });

      await test.step("wait for statistics to load", async () => {
        await landingPage.waitForStatistics();

        // Statistics should be visible (even if zero)
        const hasStats = await landingPage.hasStatistics();
        expect(hasStats).toBe(true);
      });
    });
  });

  test("header navigation - search link", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "nav-search" }, async (baseUrl) => {
      const header = new HeaderComponent(page);

      await test.step("start on landing page", async () => {
        await page.goto(baseUrl);
        await page.waitForLoadState("networkidle");
      });

      await test.step("click search navigation link", async () => {
        await header.navigateToSearch();
      });

      await test.step("verify navigation to crates page", async () => {
        expect(page.url()).toContain("/crates");

        const cratesPage = new CratesPage(page);
        await expect(cratesPage.searchInput).toBeVisible();
      });
    });
  });

  test("header navigation - logo returns home", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "nav-logo" }, async (baseUrl) => {
      const header = new HeaderComponent(page);
      const landingPage = new LandingPage(page);

      await test.step("navigate to crates page first", async () => {
        await page.goto(`${baseUrl}/crates`);
        await page.waitForLoadState("networkidle");
        expect(page.url()).toContain("/crates");
      });

      await test.step("click logo to return home", async () => {
        await header.clickLogo();
        await page.waitForLoadState("networkidle");
      });

      await test.step("verify back on landing page", async () => {
        // Verify we're on the landing page by checking for landing page content
        // URL might be "/" or "/index.html" depending on deployment
        const url = page.url();
        expect(url).not.toContain("/crates");

        // Also verify landing page branding is visible
        const hasBranding = await landingPage.hasKellnrBranding();
        expect(hasBranding).toBe(true);
      });
    });
  });

  test("header navigation - doc queue link", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "nav-docqueue" }, async (baseUrl) => {
      const header = new HeaderComponent(page);

      await test.step("start on landing page", async () => {
        await page.goto(baseUrl);
        await page.waitForLoadState("networkidle");
      });

      await test.step("click doc queue navigation link", async () => {
        await header.navigateToDocQueue();
      });

      await test.step("verify navigation to doc queue page", async () => {
        expect(page.url()).toContain("/docqueue");
      });
    });
  });

  test("theme toggle switches between light and dark mode", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "theme-toggle" }, async (baseUrl) => {
      const header = new HeaderComponent(page);

      await test.step("navigate to landing page", async () => {
        await page.goto(baseUrl);
        await page.waitForLoadState("networkidle");
      });

      await test.step("get initial theme state", async () => {
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
    });
  });

  test("search from landing page navigates to crates", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "landing-search" }, async (baseUrl) => {
      const landingPage = new LandingPage(page);

      await test.step("navigate to landing page", async () => {
        await page.goto(baseUrl);
        await landingPage.waitForPageLoad();
      });

      await test.step("perform search", async () => {
        await landingPage.search("test-crate");
      });

      await test.step("verify navigation to crates page with search query", async () => {
        expect(page.url()).toContain("/crates");
        expect(page.url()).toContain("search=test-crate");
      });
    });
  });

  test("login button visible when not authenticated", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "login-btn-visible" }, async (baseUrl) => {
      const header = new HeaderComponent(page);

      await test.step("navigate to landing page", async () => {
        await page.goto(baseUrl);
        await page.waitForLoadState("networkidle");
      });

      await test.step("verify login button is visible", async () => {
        const isVisible = await header.isLoginButtonVisible();
        expect(isVisible).toBe(true);
      });

      await test.step("click login button navigates to login page", async () => {
        await header.clickLogin();
        expect(page.url()).toContain("/login");
      });
    });
  });
});
