import type { Page, Locator } from "@playwright/test";

/**
 * Base page object class providing common utilities for all page objects.
 *
 * All page objects should extend this class to inherit:
 * - Page reference management
 * - Common wait utilities
 * - Vuetify-specific helper methods
 *
 * Each page object waits for its own content in `waitForPageLoad`, not for an
 * idle network, which needs 500 ms without requests.
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
   * Get a text field by its label.
   */
  getTextField(label: string): Locator {
    return this.page.getByLabel(label).first();
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
    const snackbar = this.page.getByTestId("snackbar").filter({ hasText: text });
    return await snackbar.isVisible();
  }

  /**
   * Wait for a snackbar notification with the given text.
   */
  async waitForSnackbar(text: string, timeout: number = 5000): Promise<void> {
    await this.page
      .getByTestId("snackbar")
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
   * Wait for navigation to a path. The URL path must end with it, so "/" does
   * not match "/login" and an instance under a path prefix still matches.
   */
  async waitForNavigation(path: string, timeout: number = 10000): Promise<void> {
    await this.page.waitForURL((url) => url.pathname.endsWith(path), { timeout });
  }
}
