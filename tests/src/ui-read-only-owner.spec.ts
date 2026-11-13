/**
 * UI tests for a read-only crate owner.
 *
 * The backend rejects changes by read-only owners (yank, doc builds, owners,
 * crate users and crate groups), so the UI must not offer them.
 */

import path from "node:path";
import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD, loginAndWait, apiCall } from "./lib/ui-fixtures";
import { CratePage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  publishCrate,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";

const READER = { name: "reader-owner", password: "reader-owner-pwd" };

test.describe("Read-only crate owner", () => {
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    test.setTimeout(120_000);
    assertKellnrBinaryExists();

    started = await startLocalKellnr({
      name: `kellnr-read-only-${Date.now()}`,
      env: { KELLNR_DOCS__ENABLED: "true" },
    });
    baseUrl = started.baseUrl;

    const registryToken = extractRegistryTokenFromCargoConfig({
      crateDir: path.resolve(process.cwd(), "crates", "test-auth-req", "foo-bar"),
      registryName: "kellnr-test",
    });
    await publishCrate({
      cratePath: "tests/crates/test-auth-req/test_lib",
      registry: "kellnr-test",
      registryToken,
    });

    // A read-only user who owns test_lib, set up with the admin session
    const page = await browser.newPage();
    await loginAndWait(page, baseUrl, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    const user = {
      name: READER.name,
      pwd1: READER.password,
      pwd2: READER.password,
      is_admin: false,
      is_read_only: true,
    };
    expect(await apiCall(page, "POST", "/api/v1/users", user)).toBe(200);
    expect(await apiCall(page, "PUT", `/api/v1/crates/test_lib/owners/${READER.name}`)).toBe(200);
    await page.close();
  });

  test.afterAll(async () => {
    if (started) {
      await started.stop();
    }
  });

  test("sees the crate settings but cannot change them", async ({ page }) => {
    await loginAndWait(page, baseUrl, READER.name, READER.password);

    const cratePage = new CratePage(page);
    await cratePage.goto("test_lib");
    await cratePage.waitForCrateData();

    // No doc builds and no yank for read-only owners
    await expect(cratePage.buildDocsButton).toHaveCount(0);
    await cratePage.clickTab("versions");
    await expect(page.getByTestId("version-row").first()).toBeVisible();
    await expect(page.getByTestId("version-yank")).toHaveCount(0);
    await expect(page.getByTestId("version-unyank")).toHaveCount(0);

    // The settings tab is shown, its controls are disabled
    await cratePage.clickTab("settings");
    await expect(page.getByText("Only crate owners with write access and admins")).toBeVisible();
    await expect(cratePage.addOwnerButton).toBeDisabled();
    await expect(page.getByTestId("settings-owner-remove").first()).toBeDisabled();
    await expect(page.getByTestId("settings-add-user").getByRole("button", { name: "Add" })).toBeDisabled();
    await expect(page.getByTestId("settings-add-group").getByRole("button", { name: "Add" })).toBeDisabled();
  });

  test("admin still sees the actions on the same crate", async ({ page }) => {
    await loginAndWait(page, baseUrl, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);

    const cratePage = new CratePage(page);
    await cratePage.goto("test_lib");
    await cratePage.waitForCrateData();

    await expect(cratePage.buildDocsButton).toBeVisible();
    await cratePage.clickTab("versions");
    await expect(page.getByTestId("version-yank").first()).toBeVisible();
    await cratePage.clickTab("settings");
    await expect(cratePage.addOwnerButton).toBeEnabled();
  });
});
