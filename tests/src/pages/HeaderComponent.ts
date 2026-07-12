import type { Page, Locator } from "@playwright/test";
import { BasePage } from "./BasePage";

/**
 * Page object for the Header component.
 *
 * The header is present on all pages and provides:
 * - Navigation links (Search, Settings, Doc Queue, Help)
 * - Logo/home link
 * - Theme toggle
 * - Login/Logout button
 */
export class HeaderComponent extends BasePage {
  // Locators
  readonly appBar: Locator;
  readonly logo: Locator;
  readonly searchNavLink: Locator;
  readonly settingsNavLink: Locator;
  readonly docQueueNavLink: Locator;
  readonly helpNavLink: Locator;
  readonly themeToggle: Locator;
  readonly loginButton: Locator;
  readonly logoutButton: Locator;
  readonly mobileMenuButton: Locator;
  readonly navigationDrawer: Locator;

  constructor(page: Page) {
    super(page);

    this.appBar = page.getByTestId("header-app-bar");
    // Logo is now a non-clickable span with the kellnr text
    // Use Home link for navigation instead
    this.logo = page.getByTestId("header-logo");

    // Desktop navigation links - each button has a dedicated testid,
    // which avoids matching the mobile drawer duplicates
    this.searchNavLink = page.getByTestId("header-nav-search");
    this.settingsNavLink = page.getByTestId("header-nav-settings");
    this.docQueueNavLink = page.getByTestId("header-nav-docqueue");
    // Help link was removed from navigation
    const desktopNav = page.getByTestId("header-nav-desktop");
    this.helpNavLink = desktopNav.getByRole("link", { name: "Help" });

    // Theme toggle button
    this.themeToggle = page.getByTestId("theme-toggle");

    // Login/Logout buttons - scoped to app bar
    this.loginButton = this.appBar.getByRole("link", { name: "Log in" });
    this.logoutButton = this.appBar.locator("button").filter({
      has: page.locator('[class*="mdi-logout"]'),
    });

    // Mobile navigation
    this.mobileMenuButton = page.getByTestId("header-nav-toggle");
    this.navigationDrawer = page.getByTestId("header-drawer");
  }

  /**
   * Navigate to home page via the Home link in navigation.
   * Note: The logo is no longer clickable, so we use the Home nav link.
   */
  async clickLogo(): Promise<void> {
    // Use Home navigation button (v-btn with to="/") instead of clicking logo (which is now non-clickable)
    const homeLink = this.page.getByTestId("header-nav-home");
    await homeLink.click();
    // Wait for Vue Router navigation to complete
    await this.page.waitForURL("**/", { timeout: 5000 }).catch(() => {
      // URL might already be at root or navigation might be hash-based
    });
    await this.page.waitForLoadState("domcontentloaded");
  }

  /**
   * Navigate to the Search/Crates page.
   */
  async navigateToSearch(): Promise<void> {
    await this.searchNavLink.click();
    await this.waitForNavigation("/crates");
  }

  /**
   * Navigate to the Settings page.
   * Note: This may redirect to login if not authenticated.
   */
  async navigateToSettings(): Promise<void> {
    await this.settingsNavLink.click();
  }

  /**
   * Navigate to the Doc Queue page.
   */
  async navigateToDocQueue(): Promise<void> {
    await this.docQueueNavLink.click();
    await this.waitForNavigation("/docqueue");
  }

  /**
   * Click the Help link (opens in new tab).
   */
  async clickHelp(): Promise<void> {
    await this.helpNavLink.click();
  }

  /**
   * Toggle the theme (light/dark mode).
   */
  async toggleTheme(): Promise<void> {
    await this.themeToggle.click();
    // Wait for theme transition
    await this.page.waitForTimeout(300);
  }

  /**
   * Check if the current theme is dark mode.
   * Vuetify applies theme class to the v-app element.
   */
  async isDarkMode(): Promise<boolean> {
    // Check for dark theme on the main app container
    const vApp = this.page.locator(".v-application");
    const classList = await vApp.getAttribute("class");
    return classList?.includes("v-theme--dark") ?? false;
  }

  /**
   * Click the login button to navigate to the login page.
   */
  async clickLogin(): Promise<void> {
    await this.loginButton.click();
    await this.waitForNavigation("/login");
  }

  /**
   * Click the logout button.
   */
  async clickLogout(): Promise<void> {
    await this.logoutButton.click();
  }

  /**
   * Check if the user is logged in (logout button visible).
   */
  async isLoggedIn(): Promise<boolean> {
    // Check if any button with logout icon is visible
    const logoutBtn = this.page.locator("button").filter({
      has: this.page.locator('[class*="mdi-logout"]'),
    });
    return await logoutBtn.isVisible();
  }

  /**
   * Check if the login button is visible (user not logged in).
   */
  async isLoginButtonVisible(): Promise<boolean> {
    return await this.loginButton.isVisible();
  }

  /**
   * Get the logged-in username displayed in the header.
   */
  async getLoggedInUsername(): Promise<string | null> {
    if (!(await this.isLoggedIn())) {
      return null;
    }
    // The username is displayed in the logout button on larger screens
    const logoutBtn = this.page.locator("button").filter({
      has: this.page.locator('[class*="mdi-logout"]'),
    });
    const text = await logoutBtn.textContent();
    return text?.trim() ?? null;
  }

  /**
   * Open the mobile navigation drawer.
   */
  async openMobileMenu(): Promise<void> {
    await this.mobileMenuButton.click();
    await this.navigationDrawer.waitFor({ state: "visible" });
  }

  /**
   * Close the mobile navigation drawer.
   */
  async closeMobileMenu(): Promise<void> {
    // Press Escape to dismiss the temporary drawer (avoids depending on Vuetify's scrim element)
    await this.page.keyboard.press("Escape");
    await this.navigationDrawer.waitFor({ state: "hidden" });
  }

  /**
   * Check if the mobile menu button is visible (responsive layout).
   */
  async isMobileMenuVisible(): Promise<boolean> {
    return await this.mobileMenuButton.isVisible();
  }

  /**
   * Navigate using mobile menu.
   */
  async navigateViaMobileMenu(itemName: string): Promise<void> {
    await this.openMobileMenu();
    const menuItem = this.navigationDrawer.getByText(itemName);
    await menuItem.click();
  }
}
