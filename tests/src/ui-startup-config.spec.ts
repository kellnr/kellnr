/**
 * UI tests for Startup Configuration (Settings > Startup config).
 *
 * Tests:
 * - Expand all opens config sections and Collapse all closes them
 * - The license filter sections show their settings
 *
 * Uses the same local Kellnr + Playwright pattern as other ui-*.spec.ts tests.
 */

import type { Page } from "@playwright/test";
import { test, expect, loginAndWait, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";

test.describe("Startup config UI Tests", () => {
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  async function openStartupConfig(page: Page): Promise<void> {
    await loginAndWait(page, baseUrl, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await expect(page.getByTestId("user-menu-button")).toBeVisible();
    await page.goto(`${baseUrl}/settings`);

    const configNavItem = page.getByTestId("nav-startup-config");
    await expect(configNavItem).toBeVisible();
    await configNavItem.click();
    await expect(page.locator(".v-expansion-panel").filter({ hasText: "Registry Filter" })).toBeVisible();
  }

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

  test("Expand all opens config sections and Collapse all closes them", async ({ page }) => {
    await openStartupConfig(page);

    // Startup Configuration header and buttons should be visible
    await expect(
      page.locator(".section-header").filter({ hasText: "Startup config" })
    ).toBeVisible();

    const expandBtn = page.getByRole("button", { name: "Expand all" });
    const collapseBtn = page.getByRole("button", { name: "Collapse all" });
    await expect(expandBtn).toBeVisible();
    await expect(collapseBtn).toBeVisible();

    await expandBtn.click();

    const dataDirRow = page.locator(".config-row").filter({ hasText: "Data Directory" });
    await expect(dataDirRow).toBeVisible();

    await collapseBtn.click();
    await expect(dataDirRow).toBeHidden();
  });

  test("License filter sections show their settings", async ({ page }) => {
    await openStartupConfig(page);

    await page.getByRole("button", { name: "Expand all" }).click();

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
    await expect(allowed).toBeVisible();
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
