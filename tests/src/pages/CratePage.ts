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

    // Header - h1 is the crate name, version is sibling generic div, description is paragraph
    // Use more specific selector to avoid matching h1 elements in markdown content
    this.crateTitle = page.locator("h1.text-h3");
    this.crateVersion = page.locator("h1.text-h3").locator("+ div, + span, + generic").first();
    // Description is a paragraph near the h1 in the main content area
    this.crateDescription = page.locator("main p").first();

    // Tabs
    this.readmeTab = page.getByRole("tab", { name: "Readme" });
    this.aboutTab = page.getByRole("tab", { name: "About" });
    this.dependenciesTab = page.getByRole("tab", { name: "Dependencies" });
    this.versionsTab = page.getByRole("tab", { name: "Versions" });
    this.settingsTab = page.getByRole("tab", { name: "Settings" });
    this.adminTab = page.getByRole("tab", { name: "Admin" });

    // Content areas
    this.readmeContent = page.locator(".markdown-body, .v-card").filter({
      has: page.locator("pre, p, h1, h2, h3"),
    });
    this.versionCards = page.locator(".v-card").filter({
      has: page.locator('[class*="version"]'),
    });
    this.dependencyList = page
      .locator(".v-card")
      .filter({ hasText: "Dependencies" })
      .first();
    this.devDependencyList = page
      .locator(".v-card")
      .filter({ hasText: "Development Dependencies" });
    this.buildDependencyList = page
      .locator(".v-card")
      .filter({ hasText: "Build Dependencies" });

    // Sidebar - located in the right column
    this.sidebar = page.locator(".v-col").filter({ has: page.locator(".v-card") }).last();
    this.installSection = page.locator(".v-card").filter({ hasText: "Install" });
    this.copyButton = page.locator('button[aria-label*="copy"], .mdi-content-copy').first();
    this.versionDownloads = page.locator("text=Version Downloads").locator("..");
    this.totalDownloads = page.locator("text=Total Downloads").locator("..");
    // "Open documentation" is a clickable div, not a button
    this.openDocsButton = page.locator('div.cursor-pointer:has-text("Open documentation"), a:has-text("Open documentation")');
    this.buildDocsButton = page.getByRole("button", { name: /Build|Rebuild/i });

    // Settings tab
    this.ownersList = page.locator(".v-card").filter({ hasText: "Crate owners" }).locator(".v-list");
    this.addOwnerInput = page.getByPlaceholder("Username").first();
    this.addOwnerButton = page.locator(".v-card").filter({ hasText: "Add crate owner" }).getByRole("button", { name: "Add" });
    this.crateUsersList = page.locator(".v-card").filter({ hasText: "Crate users" }).locator(".v-list");
    this.addUserInput = page.locator(".v-card").filter({ hasText: "Add crate user" }).getByPlaceholder("Username");
    this.addUserButton = page.locator(".v-card").filter({ hasText: "Add crate user" }).getByRole("button", { name: "Add" });
    this.downloadRestrictedCheckbox = page.getByLabel("Crate users only are allowed to download");
    this.changeAccessButton = page.getByRole("button", { name: "Change crate access rules" });

    // Admin tab
    this.deleteVersionButton = page.getByRole("button", { name: "Delete Version" });
    this.deleteCrateButton = page.getByRole("button", { name: "Delete Crate" });

    // Snackbar
    this.snackbar = page.locator(".v-snackbar");
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
    return (await this.crateTitle.textContent()) ?? "";
  }

  /**
   * Get the displayed version.
   */
  async getVersion(): Promise<string> {
    return (await this.crateVersion.textContent()) ?? "";
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
    const link = this.page.locator("a").filter({ hasText: "Repository" });
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
    const link = this.page.locator("a").filter({ hasText: "Homepage" });
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
    const licenseLabel = this.page.locator("text=License").first();
    if (await licenseLabel.isVisible()) {
      const parent = licenseLabel.locator("..").locator("..");
      const chips = parent.locator(".v-chip");
      const count = await chips.count();
      if (count > 0) {
        const licenses: string[] = [];
        for (let i = 0; i < count; i++) {
          const text = await chips.nth(i).textContent();
          if (text) licenses.push(text.trim());
        }
        return licenses.join(", ");
      }
    }
    return null;
  }

  /**
   * Get authors from About tab.
   */
  async getAuthors(): Promise<string[]> {
    await this.clickTab("about");
    const authorsSection = this.page.locator("text=Authors").first().locator("..").locator("..");
    const chips = authorsSection.locator(".v-chip");
    const count = await chips.count();
    const authors: string[] = [];
    for (let i = 0; i < count; i++) {
      const text = await chips.nth(i).textContent();
      if (text) authors.push(text.trim());
    }
    return authors;
  }

  /**
   * Get keywords from About tab.
   */
  async getKeywords(): Promise<string[]> {
    await this.clickTab("about");
    const keywordsSection = this.page.locator("text=Keywords").first().locator("..").locator("..");
    const chips = keywordsSection.locator(".v-chip");
    const count = await chips.count();
    const keywords: string[] = [];
    for (let i = 0; i < count; i++) {
      const text = await chips.nth(i).textContent();
      if (text) keywords.push(text.trim());
    }
    return keywords;
  }

  /**
   * Get categories from About tab.
   */
  async getCategories(): Promise<string[]> {
    await this.clickTab("about");
    const categoriesSection = this.page.locator("text=Categories").first().locator("..").locator("..");
    const chips = categoriesSection.locator(".v-chip");
    const count = await chips.count();
    const categories: string[] = [];
    for (let i = 0; i < count; i++) {
      const text = await chips.nth(i).textContent();
      if (text) categories.push(text.trim());
    }
    return categories;
  }

  // === Dependencies Tab Methods ===

  /**
   * Get dependency names from Dependencies tab.
   */
  async getDependencies(): Promise<string[]> {
    await this.clickTab("dependencies");
    const deps: string[] = [];
    const depCards = this.page
      .locator(".v-card")
      .filter({ hasText: "Dependencies" })
      .first()
      .locator(".v-card-text")
      .locator("> div, > a");
    const count = await depCards.count();
    for (let i = 0; i < count; i++) {
      const text = await depCards.nth(i).textContent();
      if (text) {
        // Extract the dependency name (first word)
        const name = text.trim().split(/\s+/)[0];
        if (name) deps.push(name);
      }
    }
    return deps;
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
    const versions: string[] = [];
    // Version cards contain version numbers
    const versionElements = this.page.locator(".v-card-text").locator("div").filter({
      has: this.page.locator('[class*="text-h"], text=/^\\d+\\.\\d+/'),
    });
    const count = await versionElements.count();
    for (let i = 0; i < count; i++) {
      const text = await versionElements.nth(i).textContent();
      if (text) {
        const match = text.match(/(\d+\.\d+\.\d+)/);
        if (match) versions.push(match[1]);
      }
    }
    return versions;
  }

  /**
   * Click on a specific version to view it.
   */
  async clickVersion(version: string): Promise<void> {
    await this.clickTab("versions");
    const versionCard = this.page.locator("div, .v-card").filter({
      hasText: new RegExp(`^${version.replace(/\./g, "\\.")}\\b`),
    });
    await versionCard.first().click();
    await this.waitForPageLoad();
  }

  // === Settings Tab Methods (Admin only) ===

  /**
   * Get current crate owners.
   */
  async getOwners(): Promise<string[]> {
    await this.clickTab("settings");
    await this.page.waitForTimeout(500); // Wait for data to load
    const owners: string[] = [];
    const ownerItems = this.ownersList.locator(".v-list-item");
    const count = await ownerItems.count();
    for (let i = 0; i < count; i++) {
      const text = await ownerItems.nth(i).locator(".v-list-item-title").textContent();
      if (text) owners.push(text.trim());
    }
    return owners;
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
    const ownerItem = this.ownersList.locator(".v-list-item").filter({ hasText: username });
    await ownerItem.getByRole("button", { name: "Delete" }).click();
    await this.page.waitForTimeout(500);
  }

  /**
   * Get current crate users.
   */
  async getCrateUsers(): Promise<string[]> {
    await this.clickTab("settings");
    await this.page.waitForTimeout(500);
    const users: string[] = [];
    const userItems = this.crateUsersList.locator(".v-list-item");
    const count = await userItems.count();
    for (let i = 0; i < count; i++) {
      const text = await userItems.nth(i).locator(".v-list-item-title").textContent();
      if (text) users.push(text.trim());
    }
    return users;
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
    const installSection = this.page.getByTestId("install-section");
    const copyBtn = installSection.locator("button, .v-btn").first();
    await copyBtn.click();
    await this.page.waitForTimeout(300);
  }

  /**
   * Get version downloads count.
   */
  async getVersionDownloads(): Promise<number | null> {
    const card = this.page.locator(".v-card, div").filter({ hasText: "Version Downloads" });
    const text = await card.textContent();
    if (text) {
      const match = text.match(/(\d+)/);
      return match ? parseInt(match[1], 10) : null;
    }
    return null;
  }

  /**
   * Get total downloads count.
   */
  async getTotalDownloads(): Promise<number | null> {
    const card = this.page.locator(".v-card, div").filter({ hasText: "Total Downloads" });
    const text = await card.textContent();
    if (text) {
      const match = text.match(/(\d+)/);
      return match ? parseInt(match[1], 10) : null;
    }
    return null;
  }

  /**
   * Check if documentation link is available.
   */
  async hasDocumentation(): Promise<boolean> {
    const docsButton = this.page.getByRole("button", { name: /Open Documentation/i });
    return await docsButton.isVisible();
  }

  /**
   * Check if build docs button is visible (owner/admin only).
   */
  async canBuildDocs(): Promise<boolean> {
    const buildBtn = this.page.getByRole("button", { name: /Build|Rebuild/i });
    return await buildBtn.isVisible();
  }

  /**
   * Click build docs button.
   */
  async buildDocs(): Promise<void> {
    const buildBtn = this.page.getByRole("button", { name: /Build|Rebuild/i });
    await buildBtn.click();
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
