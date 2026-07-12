import type { Page, Locator } from "@playwright/test";
import { BasePage } from "./BasePage";

/**
 * Page object for the Toolchains management section in Settings.
 *
 * Provides methods to interact with:
 * - Toolchains list with inline channel assignment
 * - Upload form
 * - Delete toolchain/target functionality
 */
export class ToolchainsPage extends BasePage {
  // Section header and navigation
  readonly toolchainsNavItem: Locator;
  readonly sectionHeader: Locator;
  readonly toolchainCount: Locator;

  // Toolchains list
  readonly toolchainsList: Locator;
  readonly emptyState: Locator;

  // Upload form
  readonly nameInput: Locator;
  readonly versionInput: Locator;
  readonly targetInput: Locator;
  readonly dateInput: Locator;
  readonly channelSelect: Locator;
  readonly dropZone: Locator;
  readonly uploadButton: Locator;
  readonly uploadAlert: Locator;

  // Dialogs
  readonly confirmDialog: Locator;
  readonly confirmButton: Locator;
  readonly cancelButton: Locator;

  // Snackbar notification
  readonly snackbar: Locator;

  constructor(page: Page) {
    super(page);

    // Navigation - use data-testid for stability
    this.toolchainsNavItem = page.getByTestId("nav-toolchains");
    this.sectionHeader = page
      .getByTestId("toolchain-section-header")
      .filter({ hasText: "Toolchain Management" });
    this.toolchainCount = this.sectionHeader.locator(".section-count");

    // Toolchains list
    this.toolchainsList = page.getByTestId("toolchain-panels");
    this.emptyState = page
      .getByTestId("toolchain-empty-state")
      .filter({ hasText: "No toolchains uploaded yet" });

    // Upload form (text fields keep semantic label-based selectors)
    this.nameInput = page.getByLabel("Component Name");
    this.versionInput = page.getByLabel("Version");
    this.targetInput = page.getByLabel("Target Triple");
    this.dateInput = page.getByLabel("Release Date");
    this.channelSelect = page.getByRole("combobox", { name: "Channel (Optional)" });
    this.dropZone = page.getByTestId("toolchain-drop-zone");
    this.uploadButton = page.getByRole("button", { name: "Upload Toolchain" });
    this.uploadAlert = page.getByTestId("toolchain-upload-alert");

    // Dialogs (shared ConfirmDialog component testids)
    this.confirmDialog = page.getByTestId("confirm-dialog");
    this.confirmButton = page.getByTestId("confirm-dialog-confirm");
    this.cancelButton = page.getByTestId("confirm-dialog-cancel");

    // Snackbar notification
    this.snackbar = page.getByTestId("snackbar");
  }

  /**
   * Locate the expansion panel for a specific toolchain (name + version).
   * Panels share a constant testid, so we filter by the visible text.
   */
  private toolchainPanel(name: string, version: string): Locator {
    return this.page.getByTestId("toolchain-panel").filter({
      hasText: `${name} ${version}`,
    });
  }

  /**
   * Navigate to the Settings page.
   */
  async goto(): Promise<void> {
    await super.goto("/settings");
    await this.waitForPageLoad();
  }

  /**
   * Navigate to the Toolchains tab within Settings.
   */
  async gotoToolchains(): Promise<void> {
    await this.goto();
    await this.clickToolchainsTab();
  }

  /**
   * Click on the Toolchains navigation item.
   */
  async clickToolchainsTab(): Promise<void> {
    await this.toolchainsNavItem.click();
    await this.page.waitForTimeout(500);
  }

  /**
   * Check if the Toolchains nav item is visible.
   */
  async isToolchainsNavVisible(): Promise<boolean> {
    return await this.toolchainsNavItem.isVisible();
  }

  /**
   * Check if we're on the Toolchains management section.
   */
  async isOnToolchainsSection(): Promise<boolean> {
    return await this.sectionHeader.isVisible();
  }

  /**
   * Check if the empty state is shown (no toolchains uploaded).
   */
  async hasEmptyState(): Promise<boolean> {
    return await this.emptyState.isVisible();
  }

  /**
   * Get the count of toolchains shown in the header badge.
   */
  async getToolchainCountFromBadge(): Promise<number | null> {
    if (await this.toolchainCount.isVisible()) {
      const text = await this.toolchainCount.textContent();
      return text ? parseInt(text, 10) : null;
    }
    return null;
  }

  /**
   * Fill in the upload form.
   */
  async fillUploadForm(data: {
    name?: string;
    version: string;
    target: string;
    date: string;
    channel?: string;
  }): Promise<void> {
    if (data.name) {
      await this.nameInput.clear();
      await this.nameInput.fill(data.name);
    }
    await this.versionInput.fill(data.version);
    await this.targetInput.fill(data.target);
    await this.dateInput.fill(data.date);

    if (data.channel) {
      await this.channelSelect.click();
      // Vuetify renders select options in an overlay; use role-based selection
      await this.page.getByRole("option", { name: data.channel }).click();
    }
  }

  /**
   * Click the upload button.
   */
  async clickUpload(): Promise<void> {
    await this.uploadButton.click();
  }

  /**
   * Check if the upload button is enabled.
   */
  async isUploadButtonEnabled(): Promise<boolean> {
    return await this.uploadButton.isEnabled();
  }

  /**
   * Get the upload alert message (success or error).
   */
  async getUploadAlertText(): Promise<string | null> {
    if (await this.uploadAlert.isVisible()) {
      return await this.uploadAlert.textContent();
    }
    return null;
  }

  /**
   * Check if there are any toolchains in the list.
   */
  async hasToolchains(): Promise<boolean> {
    const panels = this.page.getByTestId("toolchain-panel");
    const count = await panels.count();
    return count > 0;
  }

  /**
   * Get the number of toolchains in the list.
   */
  async getToolchainCount(): Promise<number> {
    const panels = this.page.getByTestId("toolchain-panel");
    return await panels.count();
  }

  /**
   * Expand a toolchain panel to see its targets.
   */
  async expandToolchain(name: string, version: string): Promise<void> {
    const panel = this.toolchainPanel(name, version);
    await panel.click();
    await this.page.waitForTimeout(300);
  }

  /**
   * Get the targets for a toolchain (expands the panel if needed).
   */
  async getTargetsForToolchain(name: string, version: string): Promise<string[]> {
    // Expand the toolchain panel first to make targets visible
    await this.expandToolchain(name, version);

    const panel = this.toolchainPanel(name, version);
    const targets = panel.getByTestId("toolchain-target-name");
    const count = await targets.count();
    const result: string[] = [];
    for (let i = 0; i < count; i++) {
      const text = await targets.nth(i).textContent();
      if (text) result.push(text);
    }
    return result;
  }

  /**
   * Delete a target from a toolchain.
   */
  async deleteTarget(name: string, version: string, target: string): Promise<void> {
    // First expand the toolchain
    await this.expandToolchain(name, version);

    // Find the target item and click delete
    const targetItem = this.page.getByTestId("target-item").filter({ hasText: target });
    const deleteButton = targetItem.getByTestId("toolchain-target-delete");
    await deleteButton.click();

    // Confirm deletion
    await this.confirmButton.click();
    await this.page.waitForTimeout(500);
  }

  /**
   * Delete an entire toolchain with all its targets.
   */
  async deleteToolchain(name: string, version: string): Promise<void> {
    // First expand the toolchain
    await this.expandToolchain(name, version);

    // Click the "Delete All" button in the targets header
    const panel = this.toolchainPanel(name, version);
    const deleteAllButton = panel.getByTestId("toolchain-delete");
    await deleteAllButton.click();

    // Confirm deletion
    await this.confirmButton.click();
    await this.page.waitForTimeout(500);
  }

  /**
   * Get the currently assigned channel for a toolchain (from the chip in the header).
   */
  async getToolchainChannel(name: string, version: string): Promise<string | null> {
    const panel = this.toolchainPanel(name, version);
    const chip = panel.getByTestId("toolchain-channel-chip");
    if (await chip.isVisible()) {
      return await chip.textContent();
    }
    return null;
  }

  /**
   * Change the channel for a toolchain using the inline dropdown.
   */
  async setToolchainChannel(name: string, version: string, channel: string): Promise<void> {
    // First expand the toolchain
    await this.expandToolchain(name, version);

    // Find the channel select dropdown inside the panel
    const panel = this.toolchainPanel(name, version);
    const channelSelect = panel.getByTestId("toolchain-channel-select");
    await channelSelect.click();
    await this.page.waitForTimeout(200);

    // Vuetify renders select options in an overlay; use role-based selection
    await this.page.getByRole("option", { name: channel }).click();
    await this.page.waitForTimeout(500);
  }

  /**
   * Clear the channel assignment for a toolchain.
   */
  async clearToolchainChannel(name: string, version: string): Promise<void> {
    // First expand the toolchain
    await this.expandToolchain(name, version);

    // The clear icon is the only element with a button role inside the
    // channel select wrapper (Vuetify renders it as an icon with role="button")
    const panel = this.toolchainPanel(name, version);
    const clearButton = panel.getByTestId("toolchain-channel-select").getByRole("button");
    if (await clearButton.isVisible()) {
      await clearButton.click();
      await this.page.waitForTimeout(500);
    }
  }

  /**
   * Wait for a snackbar notification to appear and return its message.
   * Uses .last() to handle cases where multiple snackbars may be visible.
   */
  async waitForSnackbarAndGetText(): Promise<string | null> {
    // Use .last() to get the most recent snackbar (handles multiple visible snackbars)
    const snackbar = this.snackbar.last();
    await snackbar.waitFor({ state: "visible", timeout: 5000 });
    return await snackbar.textContent();
  }

  /**
   * Check if snackbar is showing a success message.
   */
  async isSnackbarSuccess(): Promise<boolean> {
    const snackbar = this.snackbar.last();
    if (await snackbar.isVisible()) {
      const classes = await snackbar.getAttribute("class") || "";
      return classes.includes("success") || classes.includes("bg-success");
    }
    return false;
  }

  /**
   * Close the most recent snackbar if visible.
   */
  async dismissSnackbar(): Promise<void> {
    const snackbar = this.snackbar.last();
    const closeButton = snackbar.getByRole("button", { name: "Close" });
    if (await closeButton.isVisible()) {
      await closeButton.click();
      await this.page.waitForTimeout(300);
    }
  }
}
