/**
 * UI tests for finding crates and moving around the UI.
 *
 * Tests:
 * - Install snippets name the Kellnr registry
 * - Crate tabs and settings sections are kept in the URL
 * - Crate cards only show a documentation button for crates with docs
 * - The crate list shows its size and can be sorted
 * - The landing page lists recently updated crates
 * - Auth tokens show when they were created and last used
 * - The startup config can be searched
 * - Header shows only the links that apply, with accessible names
 * - The footer ends the page and the layout does not shift for a scrollbar
 *
 * The sort order itself is covered by the database tests, which run against
 * SQLite and PostgreSQL.
 *
 * Performance: All tests share a single local Kellnr instance.
 */

import path from "node:path";
import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { CratePage, CratesPage, LandingPage, LoginPage } from "./pages";
import type { Page } from "@playwright/test";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  publishCrate,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";

test.describe("Browsing", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    test.setTimeout(120_000);

    assertKellnrBinaryExists();
    started = await startLocalKellnr({ name: `kellnr-browsing-${Date.now()}` });
    baseUrl = started.baseUrl;

    const registry = "kellnr-test";
    const registryToken = extractRegistryTokenFromCargoConfig({
      crateDir: path.resolve(process.cwd(), "crates", "test-auth-req", "foo-bar"),
      registryName: registry,
    });
    // foo-bar is published last, so it is the most recently updated crate
    for (const crate of ["test_lib", "foo-bar"]) {
      await publishCrate({ cratePath: `tests/crates/test-auth-req/${crate}`, registry, registryToken });
    }
  });

  test.afterAll(async () => {
    if (started) {
      await started.stop();
    }
  });

  async function loginAsAdmin(page: Page) {
    await page.goto(`${baseUrl}/login`);
    await new LoginPage(page).login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await page.waitForURL(`${baseUrl}/`);
  }

  test("install snippets name the Kellnr registry", async ({ page }) => {
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    await new CratePage(page).waitForCrateData();

    await expect(page.getByTestId("install-snippet")).toHaveText(
      'foo-bar = { version = "1.0.0", registry = "kellnr" }',
    );
    await expect(page.getByTestId("install-command")).toHaveText("cargo add foo-bar --registry kellnr");
  });

  test("crate tab is kept in the URL", async ({ page }) => {
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    await cratePage.versionsTab.click();
    await expect(page).toHaveURL(/[?&]tab=versions/);

    await page.reload();
    await cratePage.waitForCrateData();
    await expect(cratePage.versionsTab).toHaveAttribute("aria-selected", "true");
    await expect(page.getByTestId("crate-versions")).toBeVisible();
  });

  test("link to the admin tab of a crate opens it after a reload", async ({ page }) => {
    await loginAsAdmin(page);
    // A full page load restores the login after the crate data
    await page.goto(`${baseUrl}/crate?name=foo-bar&tab=admin`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    await expect(cratePage.adminTab).toHaveAttribute("aria-selected", "true");
  });

  test("crate cards only link to docs that exist", async ({ page }) => {
    await page.goto(`${baseUrl}/crates`);
    await new CratesPage(page).waitForSearchResults();

    // The test crates have no docs
    const card = page.getByTestId("crate-card").filter({ hasText: "foo-bar" });
    await expect(card).toBeVisible();
    await expect(card.getByTestId("crate-card-docs-link")).toHaveCount(0);
  });

  test("crate list shows its size and can be sorted", async ({ page }) => {
    await page.goto(`${baseUrl}/crates`);
    const cratesPage = new CratesPage(page);
    await cratesPage.waitForSearchResults();
    await expect(page.getByTestId("crates-result-count")).toHaveText("2 crates");

    const sortRequest = page.waitForRequest((request) => request.url().includes("sort=updated"));
    await page.getByTestId("crates-sort").click();
    await page.getByRole("option", { name: "Recently updated" }).click();
    await sortRequest;
    await expect(page).toHaveURL(/[?&]sort=updated/);
    await expect(page.getByTestId("crate-card-name").first()).toHaveText("foo-bar");

    // A search is ordered by relevance and counts its matches
    await cratesPage.search("hyphens");
    await expect(page.getByTestId("crates-result-count")).toHaveText('1 crate found for "hyphens"');
    await expect(page.getByTestId("crates-sort")).toHaveCount(0);
    await expect(page).toHaveURL(/[?&]search=hyphens/);
  });

  test("landing page lists recently updated crates", async ({ page }) => {
    await page.goto(`${baseUrl}/`);
    await new LandingPage(page).waitForStatistics();

    const rows = page.getByTestId("recent-crate");
    await expect(rows).toHaveCount(2);
    await expect(rows.first().locator(".crate-name")).toHaveText("foo-bar");

    // Nothing was downloaded yet, so there is no download ranking
    await expect(page.getByTestId("landing-top-crates")).toHaveCount(0);

    await page.getByTestId("landing-recent-crates-view-all").click();
    await expect(page).toHaveURL(`${baseUrl}/crates?sort=updated`);
  });

  test("settings section is kept in the URL", async ({ page }) => {
    await loginAsAdmin(page);
    await page.goto(`${baseUrl}/settings`);

    await page.getByTestId("nav-auth-tokens").click();
    await expect(page).toHaveURL(/[?&]tab=tokens/);

    await page.reload();
    await expect(page.getByTestId("section-title")).toHaveText("Auth Tokens");
  });

  test("auth tokens show when they were created and last used", async ({ page }) => {
    await loginAsAdmin(page);
    await page.goto(`${baseUrl}/settings?tab=tokens`);

    // The admin token published the test crates
    const adminToken = page.getByTestId("token-item").filter({ hasText: "admin" });
    await expect(adminToken.getByTestId("token-dates")).toContainText(/Created .+ ago/);
    await expect(adminToken.getByTestId("token-dates")).toContainText(/Last used .+ ago/);

    await page.getByPlaceholder("Enter a descriptive name for the token").fill("unused");
    await page.getByRole("button", { name: "Create Token" }).click();
    const newToken = page.getByTestId("token-item").filter({ hasText: "unused" });
    await expect(newToken.getByTestId("token-dates")).toContainText("Never used");
  });

  test("startup config can be searched", async ({ page }) => {
    await loginAsAdmin(page);
    await page.goto(`${baseUrl}/settings?tab=config`);

    await page.getByTestId("config-search").locator("input").fill("port");

    // Matching sections open, and only matching settings are shown
    const rows = page.locator(".config-row:visible");
    await expect(rows.first()).toBeVisible();
    const texts = await rows.allTextContents();
    for (const text of texts) {
      expect(text.toLowerCase()).toContain("port");
    }
  });

  test("header shows only the links that apply", async ({ page }) => {
    await page.goto(`${baseUrl}/`);
    // Settings need a login, the user menu has them afterwards
    await expect(page.getByTestId("header-nav-settings")).toHaveCount(0);
    await expect(page.getByTestId("login-button")).toBeVisible();

    await page.goto(`${baseUrl}/login`);
    await expect(page.getByTestId("login-title")).toBeVisible();
    await expect(page.getByTestId("login-button")).toHaveCount(0);

    await loginAsAdmin(page);
    await expect(page.getByTestId("header-nav-settings")).toBeVisible();
  });

  test("icon buttons have accessible names", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(`${baseUrl}/`);

    await expect(page.getByTestId("header-nav-toggle")).toHaveAttribute("aria-label", "Open navigation menu");
    await expect(page.getByTestId("theme-toggle")).toHaveAttribute("aria-label", /Switch to (light|dark) mode/);
  });

  test("footer ends the page and the layout does not shift for a scrollbar", async ({ page }) => {
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    await new CratePage(page).waitForCrateData();

    const layout = await page.evaluate(() => {
      const footer = document.querySelector(".footer-container")!;
      const main = document.querySelector(".v-main")!;
      return {
        footerPosition: getComputedStyle(footer).position,
        footerTop: footer.getBoundingClientRect().top,
        mainBottom: main.getBoundingClientRect().bottom,
        gutter: getComputedStyle(document.documentElement).scrollbarGutter,
      };
    });
    expect(layout.footerPosition).not.toBe("fixed");
    expect(layout.footerTop).toBeGreaterThanOrEqual(layout.mainBottom - 1);
    expect(layout.gutter).toBe("stable");
  });
});
