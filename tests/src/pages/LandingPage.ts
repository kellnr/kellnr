import type { Page, Locator } from "@playwright/test";
import { BasePage } from "./BasePage";

/**
 * Page object for the Landing/Home page (/).
 *
 * Provides methods to interact with:
 * - Search input
 * - Statistics cards
 * - Top crates section
 * - Cached crates section (if proxy enabled)
 */
export class LandingPage extends BasePage {
  // Locators
  readonly searchInput: Locator;
  readonly loadingIndicator: Locator;
  readonly totalCratesCard: Locator;
  readonly totalVersionsCard: Locator;
  readonly totalDownloadsCard: Locator;
  readonly topCratesSection: Locator;
  readonly cachedCratesSection: Locator;

  constructor(page: Page) {
    super(page);

    this.searchInput = page.getByPlaceholder("Search for crates");
    this.loadingIndicator = page.locator(".v-progress-circular");

    // Statistics cards - identified by their labels
    this.totalCratesCard = page.locator("text=Total Crates").locator("..");
    this.totalVersionsCard = page.locator("text=Total Versions").locator("..");
    this.totalDownloadsCard = page
      .locator("text=Total Downloads")
      .locator("..");

    // Sections
    this.topCratesSection = page.locator("text=Top Downloaded Crates");
    this.cachedCratesSection = page.locator("text=Cached Crates");
  }

  /**
   * Navigate to the landing page.
   */
  async goto(): Promise<void> {
    await super.goto("/");
    await this.waitForPageLoad();
  }

  /**
   * Wait for statistics to load.
   */
  async waitForStatistics(timeout: number = 10000): Promise<void> {
    await this.loadingIndicator.waitFor({ state: "hidden", timeout });
  }

  /**
   * Check if statistics are loaded.
   */
  async hasStatistics(): Promise<boolean> {
    return (await this.totalCratesCard.isVisible()) || !(await this.isLoading());
  }

  /**
   * Check if the page is loading.
   */
  async isLoading(): Promise<boolean> {
    return await this.loadingIndicator.isVisible();
  }

  /**
   * Search for crates from the landing page.
   * This will navigate to the crates page with the search query.
   */
  async search(query: string): Promise<void> {
    await this.searchInput.fill(query);
    await this.searchInput.press("Enter");
    await this.waitForNavigation("/crates");
  }

  /**
   * Get the total crates count from statistics.
   */
  async getTotalCratesCount(): Promise<number | null> {
    try {
      const card = this.page.locator(".statistics-card, .v-card").filter({
        hasText: "Total Crates",
      });
      const numText = await card.locator(".text-h4, .text-h5").first().textContent();
      return numText ? parseInt(numText.replace(/,/g, ""), 10) : null;
    } catch {
      return null;
    }
  }

  /**
   * Get the total versions count from statistics.
   */
  async getTotalVersionsCount(): Promise<number | null> {
    try {
      const card = this.page.locator(".statistics-card, .v-card").filter({
        hasText: "Total Versions",
      });
      const numText = await card.locator(".text-h4, .text-h5").first().textContent();
      return numText ? parseInt(numText.replace(/,/g, ""), 10) : null;
    } catch {
      return null;
    }
  }

  /**
   * Get the total downloads count from statistics.
   */
  async getTotalDownloadsCount(): Promise<number | null> {
    try {
      const card = this.page.locator(".statistics-card, .v-card").filter({
        hasText: "Total Downloads",
      });
      const numText = await card.locator(".text-h4, .text-h5").first().textContent();
      return numText ? parseInt(numText.replace(/,/g, ""), 10) : null;
    } catch {
      return null;
    }
  }

  /**
   * Check if top crates section is visible.
   */
  async hasTopCratesSection(): Promise<boolean> {
    return await this.topCratesSection.isVisible();
  }

  /**
   * Check if cached crates section is visible (proxy enabled).
   */
  async hasCachedCratesSection(): Promise<boolean> {
    return await this.cachedCratesSection.isVisible();
  }

  /**
   * Check if the Kellnr logo/branding is visible.
   */
  async hasKellnrBranding(): Promise<boolean> {
    const branding = this.page.locator("text=Kellnr Crates");
    return await branding.isVisible();
  }
}
