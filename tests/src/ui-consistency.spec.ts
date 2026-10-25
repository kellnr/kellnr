/**
 * UI tests for consistent wording and styling across pages.
 *
 * Tests:
 * - Settings navigation and section headings use the same names
 * - Login wording uses "Log in" everywhere
 * - Count badges are shown also when the count is 0
 * - Add and remove buttons look the same in all settings lists
 * - Crate page cards share one corner radius
 * - Dependency lists use the common section header
 * - Publish docs page uses the common card and code block layout
 * - Code block copy button reports success and failure
 * - Smaller copy fixes (Change password, Startup config, Admin tab)
 *
 * Performance: All tests share a single local Kellnr instance.
 */

import path from "node:path";
import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import type { Page } from "@playwright/test";
import { CratePage, LoginPage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  publishCrate,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";

async function publishTestCrates(): Promise<void> {
  const registry = "kellnr-test";
  const registryToken = extractRegistryTokenFromCargoConfig({
    crateDir: path.resolve(process.cwd(), "crates", "test-auth-req", "foo-bar"),
    registryName: registry,
  });

  for (const crate of ["test_lib", "foo-bar"]) {
    await publishCrate({
      cratePath: `tests/crates/test-auth-req/${crate}`,
      registry,
      registryToken,
    });
  }
}

test.describe("UI Consistency", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  async function loginAsAdmin(page: Page): Promise<void> {
    const loginPage = new LoginPage(page);
    await page.goto(`${baseUrl}/login`);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    // Wait for the redirect to the landing page; a glob would already match /login
    await page.waitForURL(`${baseUrl}/`);
  }

  async function openCrateTab(page: Page, tab: "settings" | "admin" | "dependencies"): Promise<void> {
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();
    await cratePage.clickTab(tab);
  }

  test.beforeAll(async () => {
    test.setTimeout(120_000);

    assertKellnrBinaryExists();
    started = await startLocalKellnr({ name: `kellnr-consistency-${Date.now()}` });
    baseUrl = started.baseUrl;

    console.log("[setup] Publishing test crates");
    await publishTestCrates();
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      await started.stop();
    }
  });

  test("settings navigation and headings use the same names", async ({ page }) => {
    await loginAsAdmin(page);
    await page.goto(`${baseUrl}/settings`);

    const expected: Record<string, string> = {
      "nav-change-password": "Change password",
      "nav-auth-tokens": "Auth tokens",
      "nav-user-management": "Users",
      "nav-groups": "Groups",
      "nav-startup-config": "Startup config",
    };

    for (const [testId, name] of Object.entries(expected)) {
      const navItem = page.getByTestId(testId);
      await expect(navItem).toHaveText(name);
      await navItem.click();
      await expect(page.getByTestId("section-title").first()).toHaveText(name);
    }
  });

  test("login page says Log in", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await expect(loginPage.confirmButton).toHaveText("Log in");
    await expect(page.getByText("Log in to your Kellnr account")).toBeVisible();
    await expect(page.getByText(/sign in/i)).toHaveCount(0);
  });

  test("count badges show 0", async ({ page }) => {
    await loginAsAdmin(page);
    await page.goto(`${baseUrl}/settings`);
    await page.getByTestId("nav-groups").click();

    const header = page.locator(".section-header").filter({ hasText: "Groups" });
    await expect(header.locator(".section-count")).toHaveText("0");
  });

  test("crate settings use the same add and remove buttons", async ({ page }) => {
    await loginAsAdmin(page);
    await openCrateTab(page, "settings");

    for (const form of ["settings-add-owner", "settings-add-user", "settings-add-group"]) {
      const button = page.getByTestId(form).getByRole("button");
      await expect(button).toHaveText("Add");

      // The button is as tall as the input next to it
      const inputBox = await page.getByTestId(form).locator(".v-field").boundingBox();
      const buttonBox = await button.boundingBox();
      expect(Math.abs(inputBox!.height - buttonBox!.height)).toBeLessThanOrEqual(2);
    }

    // The group field uses a placeholder like the other fields
    await expect(page.getByTestId("settings-add-group").getByPlaceholder("Select or enter group")).toBeVisible();

    // Add a crate user and check its remove button looks like the owner's
    await page.getByTestId("settings-add-user").getByPlaceholder("Enter username").fill(DEFAULT_ADMIN_USER);
    await page.getByTestId("settings-add-user").getByRole("button", { name: "Add" }).click();
    const userRemove = page.getByTestId("settings-user-remove");
    await expect(userRemove).toHaveText("Remove");
    await expect(page.getByTestId("settings-owner-remove")).toHaveText("Remove");
  });

  test("crate page cards share one corner radius", async ({ page }) => {
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    const radii = await page.evaluate(() =>
      [".content-card", "[data-testid=crate-sidebar]"].map((selector) => {
        const el = document.querySelector(selector);
        return el ? getComputedStyle(el).borderTopLeftRadius : null;
      }),
    );
    expect(radii).toEqual(["12px", "12px"]);
  });

  test("dependency lists use the section header with a count", async ({ page }) => {
    await openCrateTab(page, "dependencies");

    const header = page.getByTestId("crate-deps-normal").locator(".section-header");
    await expect(header.getByTestId("section-title")).toHaveText("Dependencies");
    await expect(header.locator(".section-count")).toHaveText(/^\d+$/);
  });

  test("publish docs page uses card and code blocks", async ({ page }) => {
    await page.goto(`${baseUrl}/publishdocs`);

    const card = page.getByTestId("publish-docs");
    await expect(card.getByTestId("section-title")).toHaveText("Publish crate documentation");
    await expect(card.getByTestId("code-block")).toHaveCount(2);
    await expect(card.getByTestId("code-block-copy")).toHaveCount(2);
  });

  test("code block copy button reports success and failure", async ({ page }) => {
    // Replace the clipboard API so the result does not depend on browser
    // permissions; the first write succeeds, the second fails like on plain HTTP
    await page.addInitScript(() => {
      let calls = 0;
      Object.defineProperty(navigator, "clipboard", {
        value: {
          writeText: () => (++calls === 1 ? Promise.resolve() : Promise.reject(new Error("not allowed"))),
        },
      });
    });
    await page.goto(`${baseUrl}/publishdocs`);

    const copyButtons = page.getByTestId("code-block-copy");
    await copyButtons.nth(0).click();
    await expect(copyButtons.nth(0).locator(".mdi-check")).toBeVisible();
    await copyButtons.nth(1).click();
    await expect(copyButtons.nth(1).locator(".mdi-alert-circle-outline")).toBeVisible();
  });

  test("change password shows the New password label once", async ({ page }) => {
    await loginAsAdmin(page);
    await page.goto(`${baseUrl}/settings`);
    await page.getByTestId("nav-change-password").click();

    await expect(page.getByText("New password", { exact: true })).toHaveCount(1);
  });

  test("startup config uses singular for one setting", async ({ page }) => {
    await loginAsAdmin(page);
    await page.goto(`${baseUrl}/settings`);
    await page.getByTestId("nav-startup-config").click();
    await expect(page.getByTestId("section-title").first()).toHaveText("Startup config");

    // Sections without configured values show their number of settings
    await expect(page.getByText(/^\d+ settings?$/).first()).toBeVisible();
    await expect(page.getByText(/\b1 settings\b/)).toHaveCount(0);
  });

  test("admin tab yank hint shows a single icon", async ({ page }) => {
    await loginAsAdmin(page);
    await openCrateTab(page, "admin");

    const hint = page.locator(".v-alert").filter({ hasText: "yanking" });
    await expect(hint).toBeVisible();
    await expect(hint.locator(".v-icon")).toHaveCount(1);
  });
});
