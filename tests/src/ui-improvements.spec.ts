/**
 * UI tests for smaller usability improvements.
 *
 * Tests:
 * - Crate header fits its content
 * - Login and password fields support password managers
 * - API docs link back to the web UI
 *
 * The search ranking (exact name matches first) is covered by the database
 * tests, which run against SQLite and PostgreSQL.
 *
 * Performance: All tests share a single local Kellnr instance.
 */

import path from "node:path";
import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { CratePage, LoginPage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  publishCrate,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";

test.describe("UI Improvements", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    test.setTimeout(120_000);

    assertKellnrBinaryExists();
    started = await startLocalKellnr({ name: `kellnr-improvements-${Date.now()}` });
    baseUrl = started.baseUrl;

    const registry = "kellnr-test";
    const registryToken = extractRegistryTokenFromCargoConfig({
      crateDir: path.resolve(process.cwd(), "crates", "test-auth-req", "foo-bar"),
      registryName: registry,
    });
    for (const crate of ["test_lib", "foo-bar"]) {
      await publishCrate({ cratePath: `tests/crates/test-auth-req/${crate}`, registry, registryToken });
    }
  });

  test.afterAll(async () => {
    if (started) {
      await started.stop();
    }
  });

  test("crate header fits its content", async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    const layout = await page.evaluate(() => {
      const rect = (selector: string) => document.querySelector(selector)!.getBoundingClientRect();
      const header = rect(".crate-header");
      const title = rect("[data-testid=crate-title]");
      const description = rect("[data-testid=crate-description]");
      const tabs = rect(".crate-tabs");
      return {
        topGap: title.top - header.top,
        titleToDescription: description.top - title.bottom,
        descriptionToTabs: tabs.top - description.bottom,
      };
    });

    // No extra heading margins between the parts of the header
    expect(layout.topGap).toBeLessThanOrEqual(8);
    expect(layout.titleToDescription).toBeLessThanOrEqual(16);
    expect(layout.descriptionToTabs).toBeLessThanOrEqual(24);
  });

  test("long crate names wrap on a phone", async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 700 });
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    // Simulate a long name; the title must stay inside the header
    const fits = await page.getByTestId("crate-title").evaluate((el) => {
      el.textContent = "a-very-long-crate-name-that-does-not-fit-on-a-phone";
      const header = el.closest(".crate-header")!.getBoundingClientRect();
      return el.getBoundingClientRect().right <= header.right;
    });
    expect(fits).toBe(true);
  });

  test("login fields support password managers", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);

    await expect(page.getByTestId("login-username").locator("input")).toHaveAttribute("autocomplete", "username");
    await expect(page.getByTestId("login-password").locator("input")).toHaveAttribute(
      "autocomplete",
      "current-password",
    );

    // No empty message area between the password field and "Remember me".
    // The gap is the group margin plus the checkbox touch area; the reserved
    // message area used to add about 22px on top of that.
    const passwordBox = await page.getByTestId("login-password").boundingBox();
    const rememberBox = await loginPage.rememberMeCheckbox.boundingBox();
    expect(rememberBox!.y - (passwordBox!.y + passwordBox!.height)).toBeLessThanOrEqual(32);
  });

  test("change password fields support password managers", async ({ page }) => {
    const loginPage = new LoginPage(page);
    await page.goto(`${baseUrl}/login`);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await page.waitForURL(`${baseUrl}/`);
    await page.goto(`${baseUrl}/settings`);

    const autocomplete = await page
      .locator("input[type=password]")
      .evaluateAll((inputs) => inputs.map((input) => input.getAttribute("autocomplete")));
    expect(autocomplete).toEqual(["current-password", "new-password", "new-password"]);
  });

  test("API docs link back to the web UI", async ({ page }) => {
    await page.goto(`${baseUrl}/`);
    const href = await page.getByTestId("header-nav-desktop").getByRole("link", { name: /API Docs/i }).getAttribute("href");
    expect(href).toBe("./api/docs/");

    await page.goto(`${baseUrl}/api/docs/`);
    const backLink = page.getByRole("link", { name: "Back to Kellnr" });
    await expect(backLink).toBeVisible({ timeout: 15000 });
    // Swagger UI opens links from the description in a new tab
    const [appPage] = await Promise.all([page.context().waitForEvent("page"), backLink.click()]);
    await appPage.waitForLoadState();
    expect(appPage.url()).toBe(`${baseUrl}/`);
  });
});
