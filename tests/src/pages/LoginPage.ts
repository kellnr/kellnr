import type { Page, Locator } from "@playwright/test";
import { BasePage } from "./BasePage";

/**
 * Page object for the Login page (/login).
 *
 * Provides methods to interact with:
 * - Username input field
 * - Password input field
 * - Remember me checkbox
 * - Confirm/Submit button
 * - Login status alerts
 */
export class LoginPage extends BasePage {
  // Locators
  readonly usernameInput: Locator;
  readonly passwordInput: Locator;
  readonly rememberMeCheckbox: Locator;
  readonly confirmButton: Locator;
  readonly signInTitle: Locator;
  readonly alertMessage: Locator;

  constructor(page: Page) {
    super(page);

    // Use label-based selectors for Vuetify text fields
    this.usernameInput = page.getByLabel("User");
    this.passwordInput = page.getByLabel("Password");
    this.rememberMeCheckbox = page.getByLabel("Remember me");
    this.confirmButton = page.getByRole("button", { name: "Confirm" });
    // Sign In is rendered as v-card-title (not a semantic heading)
    this.signInTitle = page.locator(".v-card-title").filter({ hasText: "Sign In" });
    this.alertMessage = page.locator(".v-alert");
  }

  /**
   * Navigate to the login page.
   */
  async goto(): Promise<void> {
    await super.goto("/login");
    await this.waitForPageLoad();
  }

  /**
   * Check if we're on the login page.
   */
  async isOnLoginPage(): Promise<boolean> {
    return await this.signInTitle.isVisible();
  }

  /**
   * Fill in the username field.
   */
  async fillUsername(username: string): Promise<void> {
    await this.usernameInput.fill(username);
  }

  /**
   * Fill in the password field.
   */
  async fillPassword(password: string): Promise<void> {
    await this.passwordInput.fill(password);
  }

  /**
   * Toggle the "Remember me" checkbox.
   */
  async toggleRememberMe(): Promise<void> {
    await this.rememberMeCheckbox.click();
  }

  /**
   * Click the Confirm button to submit the login form.
   */
  async clickConfirm(): Promise<void> {
    await this.confirmButton.click();
  }

  /**
   * Perform a complete login flow.
   *
   * @param username - The username to log in with
   * @param password - The password to log in with
   * @param rememberMe - Whether to check the "Remember me" checkbox
   */
  async login(
    username: string,
    password: string,
    rememberMe: boolean = false,
  ): Promise<void> {
    await this.fillUsername(username);
    await this.fillPassword(password);

    if (rememberMe) {
      await this.toggleRememberMe();
    }

    await this.clickConfirm();
  }

  /**
   * Login and wait for successful redirect to home page.
   */
  async loginAndWaitForRedirect(
    username: string,
    password: string,
  ): Promise<void> {
    await this.login(username, password);
    await this.waitForNavigation("/");
  }

  /**
   * Get the alert message text (success or error).
   */
  async getAlertText(): Promise<string | null> {
    if (await this.alertMessage.isVisible()) {
      return await this.alertMessage.textContent();
    }
    return null;
  }

  /**
   * Check if a success alert is visible.
   */
  async hasSuccessAlert(): Promise<boolean> {
    // Vuetify 3 uses different alert type classes
    const alert = this.page.locator(".v-alert").filter({ hasText: "successful" });
    return await alert.isVisible();
  }

  /**
   * Check if an error alert is visible.
   */
  async hasErrorAlert(): Promise<boolean> {
    // Look for alert with error-related text
    const alert = this.page.locator(".v-alert").filter({ hasText: /wrong|error|failed/i });
    return await alert.isVisible();
  }

  /**
   * Wait for the login error message to appear.
   */
  async waitForLoginError(timeout: number = 5000): Promise<void> {
    // Wait for any alert containing error text
    await this.page
      .locator(".v-alert")
      .filter({ hasText: /wrong|error|failed/i })
      .waitFor({ state: "visible", timeout });
  }

  /**
   * Wait for the login success message to appear.
   */
  async waitForLoginSuccess(timeout: number = 5000): Promise<void> {
    await this.page
      .locator(".v-alert")
      .filter({ hasText: /success/i })
      .waitFor({ state: "visible", timeout });
  }

  /**
   * Check if the Confirm button is enabled.
   */
  async isConfirmButtonEnabled(): Promise<boolean> {
    return await this.confirmButton.isEnabled();
  }
}
