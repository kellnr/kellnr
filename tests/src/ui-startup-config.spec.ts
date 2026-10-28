/**
 * UI tests for Startup Configuration (Settings > Startup config).
 *
 * Tests:
 * - Expand all opens all config sections
 * - Collapse all closes all config sections
 * - The license filter sections show their settings
 *
 * Uses the same local Kellnr + Playwright pattern as other ui-*.spec.ts tests.
 */

import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { LoginPage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";

test.describe("Startup config UI Tests", () => {
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    test.setTimeout(60_000);

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    const suffix = `${Date.now()}`;

    started = await startLocalKellnr({
      name: `kellnr-startup-config-${suffix}`,
      env: {
        KELLNR_REGISTRY__AUTH_REQUIRED: "true",
        KELLNR_CRATESIO_FILTER__LICENSE_ENABLED: "true",
        KELLNR_CRATESIO_FILTER__LICENSE_ALLOWED: "MIT,Apache-2.0",
      },
    });

    baseUrl = started.baseUrl;
    console.log("[setup] Server ready at", baseUrl);
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
    }
  });

  test("Expand all opens config sections", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    await page.waitForTimeout(1000);

    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const configNavItem = page.getByTestId("nav-startup-config");
    await expect(configNavItem).toBeVisible();
    await configNavItem.click();
    await page.waitForTimeout(500);

    // Startup Configuration header and buttons should be visible
    await expect(
      page.locator(".section-header").filter({ hasText: "Startup config" })
    ).toBeVisible();

    const expandBtn = page.getByRole("button", { name: "Expand all" });
    const collapseBtn = page.getByRole("button", { name: "Collapse all" });
    await expect(expandBtn).toBeVisible();
    await expect(collapseBtn).toBeVisible();

    // Initially panels may be closed; click Expand all
    await expandBtn.click();
    await page.waitForTimeout(300);

    // After expand all, at least one section content should be visible (e.g. Registry "Data Directory")
    const dataDirRow = page.locator(".config-row").filter({ hasText: "Data Directory" });
    await expect(dataDirRow).toBeVisible();
  });

  test("Collapse all closes config sections", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    await page.waitForTimeout(1000);

    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const configNavItem = page.getByTestId("nav-startup-config");
    await expect(configNavItem).toBeVisible();
    await configNavItem.click();
    await page.waitForTimeout(500);

    const expandBtn = page.getByRole("button", { name: "Expand all" });
    const collapseBtn = page.getByRole("button", { name: "Collapse all" });
    await expandBtn.click();
    await page.waitForTimeout(300);

    // Content should be visible after expand
    const dataDirRow = page.locator(".config-row").filter({ hasText: "Data Directory" });
    await expect(dataDirRow).toBeVisible();

    await collapseBtn.click();
    await page.waitForTimeout(300);

    // After collapse all, panel content should be hidden (Vuetify collapses the panel body)
    await expect(dataDirRow).toBeHidden();
  });

  test("License filter sections show their settings", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    await page.waitForTimeout(1000);

    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    const configNavItem = page.getByTestId("nav-startup-config");
    await expect(configNavItem).toBeVisible();
    await configNavItem.click();
    await page.waitForTimeout(500);

    await page.getByRole("button", { name: "Expand all" }).click();
    await page.waitForTimeout(300);

    const cratesioPanel = page
      .locator(".v-expansion-panel")
      .filter({ hasText: "Crates.io Filter" });
    const registryPanel = page
      .locator(".v-expansion-panel")
      .filter({ hasText: "Registry Filter" });
    await expect(cratesioPanel).toBeVisible();
    await expect(registryPanel).toBeVisible();

    // Values set through environment variables, with the list split
    const allowed = cratesioPanel
      .locator(".config-row")
      .filter({ hasText: "Allowed Licenses" });
    await expect(allowed).toContainText("MIT");
    await expect(allowed).toContainText("Apache-2.0");
    await expect(allowed).toContainText("KELLNR_CRATESIO_FILTER__LICENSE_ALLOWED");
    await expect(
      cratesioPanel.locator(".config-row").filter({ hasText: "License Filter Enabled" })
    ).toContainText("true");

    // Defaults of the registry filter
    await expect(
      registryPanel.locator(".config-row").filter({ hasText: "Unknown License" })
    ).toContainText("deny");
    await expect(
      registryPanel.locator(".config-row").filter({ hasText: "License-Exempt Crates" })
    ).toContainText("--registry-filter-license-exempt-crates");
  });
});
