import type { Page, Locator } from "@playwright/test";
import { BasePage } from "./BasePage";

/**
 * Page object for the individual Crate detail page (/crate?name=xxx).
 *
 * Provides methods to interact with:
 * - Crate header (name, version, description)
 * - Tabs (Readme, About, Dependencies, Versions, Settings, Admin)
 * - Sidebar (install info, downloads, documentation)
 * - Owner management
 * - Access control settings
 * - Admin actions (delete)
 *
 * All locators are based on `data-testid` attributes defined in the
 * crate-detail Vue components (Crate.vue, CrateSidebar.vue, About.vue,
 * Version.vue, Dependency.vue, Readme.vue, CrateSettingsTab.vue,
 * CrateAdminTab.vue).
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
  readonly settingsTab: Locator;
  readonly adminTab: Locator;

  // Content locators
  readonly readmeContent: Locator;
  readonly versionCards: Locator;
  readonly dependencyList: Locator;
  readonly devDependencyList: Locator;
  readonly buildDependencyList: Locator;

  // Sidebar locators
  readonly sidebar: Locator;
  readonly installSection: Locator;
  readonly copyButton: Locator;
  readonly versionDownloads: Locator;
  readonly totalDownloads: Locator;
  readonly openDocsButton: Locator;
  readonly buildDocsButton: Locator;

  // Settings tab locators
  readonly ownersList: Locator;
  readonly addOwnerInput: Locator;
  readonly addOwnerButton: Locator;
  readonly crateUsersList: Locator;
  readonly addUserInput: Locator;
  readonly addUserButton: Locator;
  readonly downloadRestrictedCheckbox: Locator;
  readonly changeAccessButton: Locator;

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
    this.settingsTab = page.getByTestId("crate-tab-settings");
    this.adminTab = page.getByTestId("crate-tab-admin");

    // Content areas
    this.readmeContent = page.getByTestId("crate-readme");
    this.versionCards = page.getByTestId("version-row");
    this.dependencyList = page.getByTestId("crate-deps-normal");
    this.devDependencyList = page.getByTestId("crate-deps-dev");
    this.buildDependencyList = page.getByTestId("crate-deps-build");

    // Sidebar
    this.sidebar = page.getByTestId("crate-sidebar");
    this.installSection = page.getByTestId("install-section");
    this.copyButton = page.getByTestId("sidebar-copy-button");
    this.versionDownloads = page.getByTestId("sidebar-version-downloads");
    this.totalDownloads = page.getByTestId("sidebar-total-downloads");
    this.openDocsButton = page.getByTestId("sidebar-open-docs");
    this.buildDocsButton = page.getByTestId("sidebar-build-docs");

    // Settings tab
    this.ownersList = page.getByTestId("settings-owners");
    this.addOwnerInput = page.getByTestId("settings-add-owner").getByRole("textbox");
    this.addOwnerButton = page.getByTestId("settings-add-owner").getByRole("button", { name: "Add" });
    this.crateUsersList = page.getByTestId("settings-users");
    this.addUserInput = page.getByTestId("settings-add-user").getByRole("textbox");
    this.addUserButton = page.getByTestId("settings-add-user").getByRole("button");
    this.downloadRestrictedCheckbox = page
      .getByTestId("settings-download-restricted")
      .locator('input[type="checkbox"]');
    this.changeAccessButton = page.getByTestId("settings-save-access");

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

  /**
   * Click on a tab by name.
   */
  async clickTab(
    tabName: "readme" | "about" | "dependencies" | "versions" | "settings" | "admin"
  ): Promise<void> {
    const tabMap = {
      readme: this.readmeTab,
      about: this.aboutTab,
      dependencies: this.dependenciesTab,
      versions: this.versionsTab,
      settings: this.settingsTab,
      admin: this.adminTab,
    };
    await tabMap[tabName].click();
    await this.page.waitForTimeout(300); // Wait for tab content to load
  }

  /**
   * Check if a tab is visible.
   */
  async isTabVisible(
    tabName: "readme" | "about" | "dependencies" | "versions" | "settings" | "admin"
  ): Promise<boolean> {
    const tabMap = {
      readme: this.readmeTab,
      about: this.aboutTab,
      dependencies: this.dependenciesTab,
      versions: this.versionsTab,
      settings: this.settingsTab,
      admin: this.adminTab,
    };
    return await tabMap[tabName].isVisible();
  }

  // === About Tab Methods ===

  /**
   * Get repository link from About tab.
   */
  async getRepositoryLink(): Promise<string | null> {
    await this.clickTab("about");
    const link = this.page.getByTestId("about-repository");
    if (await link.isVisible()) {
      return await link.getAttribute("href");
    }
    return null;
  }

  /**
   * Get homepage link from About tab.
   */
  async getHomepageLink(): Promise<string | null> {
    await this.clickTab("about");
    const link = this.page.getByTestId("about-homepage");
    if (await link.isVisible()) {
      return await link.getAttribute("href");
    }
    return null;
  }

  /**
   * Get license from About tab.
   */
  async getLicense(): Promise<string | null> {
    await this.clickTab("about");
    const license = this.page.getByTestId("about-license");
    if (await license.isVisible()) {
      const text = ((await license.textContent()) ?? "").trim();
      return text.length > 0 ? text : null;
    }
    return null;
  }

  /**
   * Get authors from About tab.
   */
  async getAuthors(): Promise<string[]> {
    await this.clickTab("about");
    const texts = await this.page.getByTestId("about-author").allTextContents();
    return texts.map((t) => t.trim()).filter((t) => t.length > 0);
  }

  /**
   * Get keywords from About tab.
   */
  async getKeywords(): Promise<string[]> {
    await this.clickTab("about");
    const texts = await this.page.getByTestId("about-keyword").allTextContents();
    return texts.map((t) => t.trim()).filter((t) => t.length > 0);
  }

  /**
   * Get categories from About tab.
   */
  async getCategories(): Promise<string[]> {
    await this.clickTab("about");
    const texts = await this.page.getByTestId("about-category").allTextContents();
    return texts.map((t) => t.trim()).filter((t) => t.length > 0);
  }

  // === Dependencies Tab Methods ===

  /**
   * Get dependency names from Dependencies tab.
   */
  async getDependencies(): Promise<string[]> {
    await this.clickTab("dependencies");
    const names = await this.dependencyList.getByTestId("dependency-name").allTextContents();
    return names.map((n) => n.trim()).filter((n) => n.length > 0);
  }

  /**
   * Check if dev dependencies section is visible.
   */
  async hasDevDependencies(): Promise<boolean> {
    await this.clickTab("dependencies");
    return await this.devDependencyList.isVisible();
  }

  /**
   * Check if build dependencies section is visible.
   */
  async hasBuildDependencies(): Promise<boolean> {
    await this.clickTab("dependencies");
    return await this.buildDependencyList.isVisible();
  }

  // === Versions Tab Methods ===

  /**
   * Get all version strings from Versions tab.
   */
  async getVersions(): Promise<string[]> {
    await this.clickTab("versions");
    const texts = await this.page.getByTestId("version-number").allTextContents();
    return texts.map((t) => t.trim()).filter((t) => t.length > 0);
  }

  /**
   * Click on a specific version to view it.
   */
  async clickVersion(version: string): Promise<void> {
    await this.clickTab("versions");
    const versionRow = this.page.getByTestId("version-row").filter({
      has: this.page.getByText(version, { exact: true }),
    });
    await versionRow.first().click();
    await this.waitForPageLoad();
  }

  // === Settings Tab Methods (Admin only) ===

  /**
   * Get current crate owners.
   */
  async getOwners(): Promise<string[]> {
    await this.clickTab("settings");
    await this.page.waitForTimeout(500); // Wait for data to load
    const names = await this.page.getByTestId("settings-owner-name").allTextContents();
    return names.map((n) => n.trim()).filter((n) => n.length > 0);
  }

  /**
   * Add a crate owner.
   */
  async addOwner(username: string): Promise<void> {
    await this.clickTab("settings");
    await this.addOwnerInput.fill(username);
    await this.addOwnerButton.click();
    await this.page.waitForTimeout(500);
  }

  /**
   * Delete a crate owner.
   */
  async deleteOwner(username: string): Promise<void> {
    await this.clickTab("settings");
    // Handle confirmation dialog
    this.page.on("dialog", (dialog) => dialog.accept());
    const ownerRow = this.page.getByTestId("settings-owner-row").filter({ hasText: username });
    await ownerRow.getByTestId("settings-owner-remove").click();
    await this.page.waitForTimeout(500);
  }

  /**
   * Get current crate users.
   */
  async getCrateUsers(): Promise<string[]> {
    await this.clickTab("settings");
    await this.page.waitForTimeout(500);
    const names = await this.page.getByTestId("settings-user-name").allTextContents();
    return names.map((n) => n.trim()).filter((n) => n.length > 0);
  }

  /**
   * Add a crate user.
   */
  async addCrateUser(username: string): Promise<void> {
    await this.clickTab("settings");
    await this.addUserInput.fill(username);
    await this.addUserButton.click();
    await this.page.waitForTimeout(500);
  }

  /**
   * Toggle download restriction.
   */
  async toggleDownloadRestriction(): Promise<void> {
    await this.clickTab("settings");
    await this.downloadRestrictedCheckbox.click();
    await this.changeAccessButton.click();
    await this.page.waitForTimeout(500);
  }

  /**
   * Check if download is restricted.
   */
  async isDownloadRestricted(): Promise<boolean> {
    await this.clickTab("settings");
    return await this.downloadRestrictedCheckbox.isChecked();
  }

  // === Admin Tab Methods ===

  /**
   * Delete the current version (with confirmation).
   */
  async deleteCurrentVersion(): Promise<void> {
    await this.clickTab("admin");
    // Handle confirmation dialog
    this.page.on("dialog", (dialog) => dialog.accept());
    await this.deleteVersionButton.click();
    await this.waitForNavigation("/crates");
  }

  /**
   * Delete the entire crate (with confirmation).
   */
  async deleteEntireCrate(): Promise<void> {
    await this.clickTab("admin");
    // Handle confirmation dialog
    this.page.on("dialog", (dialog) => dialog.accept());
    await this.deleteCrateButton.click();
    await this.waitForNavigation("/crates");
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
   * Copy install snippet to clipboard.
   */
  async copyInstallSnippet(): Promise<void> {
    await this.copyButton.click();
    await this.page.waitForTimeout(300);
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

  /**
   * Check if the crate page loaded successfully.
   */
  async isLoaded(): Promise<boolean> {
    try {
      await this.crateTitle.waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }
}
