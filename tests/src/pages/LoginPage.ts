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
 * - OAuth2/SSO login button (when enabled)
 */
export class LoginPage extends BasePage {
  // Locators
  readonly usernameInput: Locator;
  readonly passwordInput: Locator;
  readonly rememberMeCheckbox: Locator;
  readonly confirmButton: Locator;
  readonly signInTitle: Locator;
  readonly alertMessage: Locator;
  readonly oauth2Button: Locator;
  readonly oauth2Divider: Locator;

  constructor(page: Page) {
    super(page);

    // Use placeholder-based selectors for the custom login form
    this.usernameInput = page.getByPlaceholder("Enter your username");
    this.passwordInput = page.getByPlaceholder("Enter your password");
    this.rememberMeCheckbox = page.getByLabel("Remember me");
    this.confirmButton = page.getByRole("button", { name: "Sign In" });
    // Title is now an h1 with class login-title
    this.signInTitle = page.locator(".login-title");
    this.alertMessage = page.locator(".v-alert");
    // OAuth2/SSO button (uses the oauth2-button class)
    this.oauth2Button = page.locator(".oauth2-button");
    this.oauth2Divider = page.locator(".oauth2-divider");
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

  /**
   * Check if the OAuth2/SSO button is visible.
   */
  async isOAuth2ButtonVisible(): Promise<boolean> {
    return await this.oauth2Button.isVisible();
  }

  /**
   * Get the OAuth2 button text.
   */
  async getOAuth2ButtonText(): Promise<string | null> {
    if (await this.oauth2Button.isVisible()) {
      return await this.oauth2Button.textContent();
    }
    return null;
  }

  /**
   * Click the OAuth2/SSO login button.
   * This will redirect to the OAuth2 provider.
   */
  async clickOAuth2Login(): Promise<void> {
    await this.oauth2Button.click();
  }

  /**
   * Wait for OAuth2 button to be visible.
   */
  async waitForOAuth2Button(timeout: number = 5000): Promise<void> {
    await this.oauth2Button.waitFor({ state: "visible", timeout });
  }
}
