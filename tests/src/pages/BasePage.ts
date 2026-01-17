import type { Page, Locator } from "@playwright/test";

/**
 * Base page object class providing common utilities for all page objects.
 *
 * All page objects should extend this class to inherit:
 * - Page reference management
 * - Common wait utilities
 * - Vuetify-specific helper methods
 */
export abstract class BasePage {
  constructor(protected readonly page: Page) {}

  /**
   * Navigate to a path relative to the base URL.
   */
  async goto(path: string = "/"): Promise<void> {
    await this.page.goto(path);
  }

  /**
   * Wait for the page to be fully loaded (network idle).
   */
  async waitForPageLoad(): Promise<void> {
    await this.page.waitForLoadState("networkidle");
  }

  /**
   * Wait for a Vuetify component to be visible and stable.
   */
  async waitForVuetifyComponent(locator: Locator): Promise<void> {
    await locator.waitFor({ state: "visible" });
    // Vuetify components often have transitions, wait for stability
    await this.page.waitForTimeout(100);
  }

  /**
   * Get a Vuetify text field by its label.
   */
  getTextField(label: string): Locator {
    return this.page.locator(`input`).filter({
      has: this.page.locator(`xpath=ancestor::*[contains(@class, "v-text-field")]//label[contains(text(), "${label}")]`),
    }).first();
  }

  /**
   * Get a Vuetify text field by label using a more reliable method.
   */
  getTextFieldByLabel(label: string): Locator {
    return this.page.getByLabel(label);
  }

  /**
   * Get a button by its text content.
   */
  getButton(text: string): Locator {
    return this.page.getByRole("button", { name: text });
  }

  /**
   * Get a link by its text content.
   */
  getLink(text: string): Locator {
    return this.page.getByRole("link", { name: text });
  }

  /**
   * Check if a snackbar notification is visible with the given text.
   */
  async hasSnackbar(text: string): Promise<boolean> {
    const snackbar = this.page.locator(".v-snackbar").filter({ hasText: text });
    return await snackbar.isVisible();
  }

  /**
   * Wait for a snackbar notification with the given text.
   */
  async waitForSnackbar(text: string, timeout: number = 5000): Promise<void> {
    await this.page
      .locator(".v-snackbar")
      .filter({ hasText: text })
      .waitFor({ state: "visible", timeout });
  }

  /**
   * Get the current URL path.
   */
  getCurrentPath(): string {
    return new URL(this.page.url()).pathname;
  }

  /**
   * Wait for navigation to a specific path.
   * Handles Vue Router SPA navigation by waiting for URL change and network idle.
   */
  async waitForNavigation(path: string, timeout: number = 10000): Promise<void> {
    await this.page.waitForURL(`**${path}**`, { timeout });
    // Wait for Vue Router to finish rendering after navigation
    await this.page.waitForLoadState("domcontentloaded");
  }
}
