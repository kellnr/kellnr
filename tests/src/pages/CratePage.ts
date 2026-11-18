import { expect, type Page, type Locator, type Response } from "@playwright/test";
import { BasePage } from "./BasePage";

export type CrateTab = "readme" | "about" | "dependencies" | "versions" | "dependents" | "settings" | "admin";

/**
 * Page object for the individual Crate detail page (/crate?name=xxx).
 *
 * Provides methods to interact with:
 * - Crate header (name, version, description)
 * - Tabs (Readme, About, Dependencies, Versions, Used by, Settings, Admin)
 * - Sidebar (install info, downloads, documentation)
 * - Access control settings
 * - Admin actions (delete)
 *
 * All locators are based on `data-testid` attributes defined in the
 * crate-detail Vue components (Crate.vue, CrateSidebar.vue, About.vue,
 * Version.vue, Dependency.vue, Readme.vue, CrateDependentsTab.vue,
 * CrateSettingsTab.vue, CrateAdminTab.vue).
 */
export class CratePage extends BasePage {
  // Header locators
  readonly crateTitle: Locator;
  readonly crateVersion: Locator;
  readonly crateDescription: Locator;

  // Tab locators
  readonly readmeTab: Locator;
  readonly aboutTab: Locator;
  readonly dependenciesTab: Locator;
  readonly versionsTab: Locator;
  readonly dependentsTab: Locator;
  readonly settingsTab: Locator;
  readonly accessBadge: Locator;
  readonly adminTab: Locator;

  // Content locators
  readonly readmeContent: Locator;
  readonly versionCards: Locator;
  readonly dependentsSummary: Locator;
  readonly dependentRows: Locator;
  readonly restrictedDependentRows: Locator;

  // Sidebar locators
  readonly sidebar: Locator;
  readonly installSection: Locator;
  readonly versionDownloads: Locator;
  readonly totalDownloads: Locator;
  readonly openDocsButton: Locator;
  readonly buildDocsButton: Locator;

  // Settings tab locators
  readonly ownersList: Locator;
  readonly addOwnerButton: Locator;
  readonly crateUsersList: Locator;
  readonly addUserInput: Locator;
  readonly addUserButton: Locator;
  readonly downloadRestrictedCheckbox: Locator;

  // Admin tab locators
  readonly deleteVersionButton: Locator;
  readonly deleteCrateButton: Locator;

  // Snackbar
  readonly snackbar: Locator;

  constructor(page: Page) {
    super(page);

    // Header
    this.crateTitle = page.getByTestId("crate-title");
    this.crateVersion = page.getByTestId("crate-version");
    this.crateDescription = page.getByTestId("crate-description");

    // Tabs
    this.readmeTab = page.getByTestId("crate-tab-readme");
    this.aboutTab = page.getByTestId("crate-tab-about");
    this.dependenciesTab = page.getByTestId("crate-tab-deps");
    this.versionsTab = page.getByTestId("crate-tab-versions");
    this.dependentsTab = page.getByTestId("crate-tab-dependents");
    this.settingsTab = page.getByTestId("crate-tab-settings");
    this.accessBadge = page.getByTestId("crate-access-badge");
    this.adminTab = page.getByTestId("crate-tab-admin");

    // Content areas
    this.readmeContent = page.getByTestId("crate-readme");
    this.versionCards = page.getByTestId("version-row");
    this.dependentsSummary = page.getByTestId("dependents-summary");
    this.dependentRows = page.getByTestId("dependent-row");
    this.restrictedDependentRows = page.getByTestId("dependent-restricted");

    // Sidebar
    this.sidebar = page.getByTestId("crate-sidebar");
    this.installSection = page.getByTestId("install-section");
    this.versionDownloads = page.getByTestId("sidebar-version-downloads");
    this.totalDownloads = page.getByTestId("sidebar-total-downloads");
    this.openDocsButton = page.getByTestId("sidebar-open-docs");
    this.buildDocsButton = page.getByTestId("sidebar-build-docs");

    // Settings tab
    this.ownersList = page.getByTestId("settings-owners");
    this.addOwnerButton = page.getByTestId("settings-add-owner").getByRole("button", { name: "Add" });
    this.crateUsersList = page.getByTestId("settings-users");
    this.addUserInput = page.getByTestId("settings-add-user").getByRole("textbox");
    this.addUserButton = page.getByTestId("settings-add-user").getByRole("button");
    this.downloadRestrictedCheckbox = page
      .getByTestId("settings-download-restricted")
      .locator('input[type="checkbox"]');

    // Admin tab
    this.deleteVersionButton = page.getByTestId("admin-delete-version");
    this.deleteCrateButton = page.getByTestId("admin-delete-crate");

    // Snackbar
    this.snackbar = page.getByTestId("snackbar");
  }

  /**
   * Navigate to a crate's detail page.
   */
  async goto(crateName: string, version?: string): Promise<void> {
    const url = version
      ? `/crate?name=${encodeURIComponent(crateName)}&version=${encodeURIComponent(version)}`
      : `/crate?name=${encodeURIComponent(crateName)}`;
    await super.goto(url);
    await this.waitForPageLoad();
  }

  /**
   * Wait until the crate, its not-found state or its load error is shown.
   */
  async waitForPageLoad(): Promise<void> {
    await this.crateTitle
      .or(this.page.getByTestId("crate-not-found"))
      .or(this.page.getByTestId("crate-load-error"))
      .waitFor({ state: "visible" });
  }

  /**
   * Get the crate name from the page.
   */
  async getCrateName(): Promise<string> {
    return ((await this.crateTitle.textContent()) ?? "").trim();
  }

  /**
   * Get the displayed version.
   */
  async getVersion(): Promise<string> {
    return ((await this.crateVersion.textContent()) ?? "").trim();
  }

  /**
   * Get the crate description.
   */
  async getDescription(): Promise<string | null> {
    try {
      return await this.crateDescription.textContent();
    } catch {
      return null;
    }
  }

  private tab(tabName: CrateTab): Locator {
    const tabs: Record<CrateTab, Locator> = {
      readme: this.readmeTab,
      about: this.aboutTab,
      dependencies: this.dependenciesTab,
      versions: this.versionsTab,
      dependents: this.dependentsTab,
      settings: this.settingsTab,
      admin: this.adminTab,
    };
    return tabs[tabName];
  }

  private waitForCrateApiResponse(method: string, matches: (path: string) => boolean): Promise<Response> {
    return this.page.waitForResponse((response) =>
      response.request().method() === method && matches(new URL(response.url()).pathname)
    );
  }

  /**
   * Waits for the owners, users and access data the settings tab requests when
   * it opens. Requests, not responses, are matched, so the earlier requests of
   * the access badge for the same data cannot satisfy the wait.
   */
  private waitForSettingsData(): Promise<unknown> {
    const matchers = [
      (path: string) => path.endsWith("/owners"),
      (path: string) => path.endsWith("/crate_users"),
      (path: string) => path.includes("/api/v1/acl/"),
    ];
    return Promise.all(
      matchers.map(async (matches) => {
        const request = await this.page.waitForRequest((r) => r.method() === "GET" && matches(new URL(r.url()).pathname));
        const response = await request.response();
        expect(response?.ok(), `${request.url()} returned ${response?.status()}`).toBe(true);
      }),
    );
  }

  /**
   * Waits for the rendered content of a tab that loads data. Every crate has
   * an owner, so the owners list shows once the settings data has loaded.
   */
  private async waitForTabContent(tabName: CrateTab): Promise<void> {
    if (tabName === "settings") {
      await this.ownersList.waitFor({ state: "visible" });
    } else if (tabName === "dependents") {
      await expect(this.page.getByTestId("crate-dependents").locator(".loading-state")).toHaveCount(0);
    }
  }

  /**
   * Open a tab and wait until its content has loaded. A tab that is already
   * open, for example from the URL, is not clicked again.
   */
  async clickTab(tabName: CrateTab): Promise<void> {
    const tab = this.tab(tabName);
    if ((await tab.getAttribute("aria-selected")) !== "true") {
      const settingsLoaded = tabName === "settings" ? this.waitForSettingsData() : undefined;
      await tab.click();
      await expect(tab).toHaveAttribute("aria-selected", "true");
      await settingsLoaded;
    }
    await this.waitForTabContent(tabName);
  }

  /**
   * Check if a tab is visible.
   */
  async isTabVisible(tabName: CrateTab): Promise<boolean> {
    return await this.tab(tabName).isVisible();
  }

  // === About Tab Methods ===

  /**
   * Get repository link from the sidebar.
   */
  async getRepositoryLink(): Promise<string | null> {
    const link = this.page.getByTestId("sidebar-repository");
    if (await link.isVisible()) {
      return await link.getAttribute("href");
    }
    return null;
  }

  /**
   * Get homepage link from the sidebar.
   */
  async getHomepageLink(): Promise<string | null> {
    const link = this.page.getByTestId("sidebar-homepage");
    if (await link.isVisible()) {
      return await link.getAttribute("href");
    }
    return null;
  }

  /**
   * Get license from the sidebar.
   */
  async getLicense(): Promise<string | null> {
    const license = this.page.getByTestId("sidebar-license");
    if (await license.isVisible()) {
      const text = ((await license.textContent()) ?? "").trim();
      return text.length > 0 ? text : null;
    }
    return null;
  }

  /**
   * Get keywords from the crate header, without the leading "#".
   */
  async getKeywords(): Promise<string[]> {
    const texts = await this.page.getByTestId("crate-keyword").allTextContents();
    return texts.map((t) => t.trim().replace(/^#/, "")).filter((t) => t.length > 0);
  }

  /**
   * Get categories from the crate header.
   */
  async getCategories(): Promise<string[]> {
    const texts = await this.page.getByTestId("crate-category").allTextContents();
    return texts.map((t) => t.trim()).filter((t) => t.length > 0);
  }

  // === Settings Tab Methods (Admin only) ===

  /**
   * Add a crate user.
   */
  async addCrateUser(username: string): Promise<void> {
    await this.clickTab("settings");
    await this.addUserInput.fill(username);
    const added = this.waitForCrateApiResponse("PUT", (path) => path.endsWith(`/crate_users/${encodeURIComponent(username)}`));
    await this.addUserButton.click();
    const response = await added;
    expect(response.ok(), `adding crate user ${username} returned ${response.status()}`).toBe(true);
    await expect(this.page.getByTestId("settings-user-name").filter({ hasText: username })).toBeVisible();
  }

  /**
   * Toggle download restriction.
   */
  async toggleDownloadRestriction(): Promise<void> {
    await this.clickTab("settings");
    // The restriction is saved as soon as the checkbox changes
    const saved = this.waitForCrateApiResponse("PUT", (path) => path.includes("/api/v1/acl/"));
    await this.downloadRestrictedCheckbox.click();
    const response = await saved;
    expect(response.ok(), `saving the download restriction returned ${response.status()}`).toBe(true);
    await expect(this.downloadRestrictedCheckbox).toBeEnabled();
  }

  // === Admin Tab Methods ===

  /**
   * Delete the current version (with confirmation).
   */
  async deleteCurrentVersion(): Promise<void> {
    await this.clickTab("admin");
    await this.deleteVersionButton.click();
    await this.confirmDialog();
    await this.waitForNavigation("/crates");
  }

  /**
   * Delete the entire crate (with confirmation).
   */
  async deleteEntireCrate(): Promise<void> {
    await this.clickTab("admin");
    await this.deleteCrateButton.click();
    // The dialog asks for the original crate name, which the URL may spell differently
    const crateName = (await this.crateTitle.innerText()).trim();
    await this.confirmDialog(crateName);
    await this.waitForNavigation("/crates");
  }

  /**
   * Confirm the open confirmation dialog, typing the required text first if given.
   */
  async confirmDialog(requiredInput?: string): Promise<void> {
    const dialog = this.page.getByTestId("confirm-dialog");
    await expect(dialog).toBeVisible();
    if (requiredInput !== undefined) {
      await dialog.getByTestId("confirm-dialog-input").locator("input").fill(requiredInput);
    }
    await dialog.getByTestId("confirm-dialog-confirm").click();
  }

  // === Sidebar Methods ===

  /**
   * Get the install snippet text.
   */
  async getInstallSnippet(): Promise<string | null> {
    try {
      const installSnippet = this.page.getByTestId("install-snippet");
      await installSnippet.waitFor({ state: "visible", timeout: 10000 });
      return await installSnippet.textContent();
    } catch {
      return null;
    }
  }

  /**
   * Get version downloads count.
   */
  async getVersionDownloads(): Promise<number | null> {
    const text = await this.versionDownloads.textContent();
    if (text) {
      const match = text.replace(/[,.\s]/g, "").match(/(\d+)/);
      return match ? parseInt(match[1], 10) : null;
    }
    return null;
  }

  /**
   * Get total downloads count.
   */
  async getTotalDownloads(): Promise<number | null> {
    const text = await this.totalDownloads.textContent();
    if (text) {
      const match = text.replace(/[,.\s]/g, "").match(/(\d+)/);
      return match ? parseInt(match[1], 10) : null;
    }
    return null;
  }

  /**
   * Check if documentation link is available.
   */
  async hasDocumentation(): Promise<boolean> {
    return await this.openDocsButton.isVisible();
  }

  /**
   * Check if build docs button is visible (owner/admin only).
   */
  async canBuildDocs(): Promise<boolean> {
    return await this.buildDocsButton.isVisible();
  }

  /**
   * Click build docs button.
   */
  async buildDocs(): Promise<void> {
    await this.buildDocsButton.click();
    await this.waitForNavigation("/docqueue");
  }

  /**
   * Wait for crate data to load.
   */
  async waitForCrateData(timeout: number = 10000): Promise<void> {
    await this.crateTitle.waitFor({ state: "visible", timeout });
  }
}
