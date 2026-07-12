import type { Page, Locator } from "@playwright/test";
import { BasePage } from "./BasePage";

/**
 * Page object for the Crates listing page (/crates).
 *
 * Provides methods to interact with:
 * - Search input field
 * - Crates proxy toggle switch
 * - Crate cards list
 * - Loading indicators
 * - Empty state
 */
export class CratesPage extends BasePage {
  // Locators
  readonly searchInput: Locator;
  readonly cratesProxySwitch: Locator;
  readonly crateCards: Locator;
  readonly loadingIndicator: Locator;
  readonly emptyState: Locator;
  readonly endOfResults: Locator;

  constructor(page: Page) {
    super(page);

    // Vuetify renders the data-testid on the component root, so scope to the
    // inner <input> for fill/press/checked interactions
    this.searchInput = page.getByTestId("crates-search").getByRole("textbox");
    this.cratesProxySwitch = page.getByTestId("crates-proxy-toggle").getByRole("checkbox");
    this.crateCards = page.getByTestId("crate-card");
    this.loadingIndicator = page.getByTestId("crates-loading");
    this.emptyState = page.getByText("No crates found");
    this.endOfResults = page.getByText("End of crates");
  }

  /**
   * Navigate to the crates page.
   */
  async goto(): Promise<void> {
    await super.goto("/crates");
    await this.waitForPageLoad();
  }

  /**
   * Navigate to the crates page with a search query.
   */
  async gotoWithSearch(query: string): Promise<void> {
    await super.goto(`/crates?search=${encodeURIComponent(query)}`);
    await this.waitForPageLoad();
  }

  /**
   * Search for crates by name.
   */
  async search(query: string): Promise<void> {
    await this.searchInput.fill(query);
    await this.searchInput.press("Enter");
    // Wait for the search to complete
    await this.waitForSearchResults();
  }

  /**
   * Clear the search input.
   */
  async clearSearch(): Promise<void> {
    await this.searchInput.clear();
    await this.searchInput.press("Enter");
  }

  /**
   * Toggle the crates proxy switch.
   */
  async toggleCratesProxy(): Promise<void> {
    // Click the visible switch label, not the hidden input
    await this.page.getByTestId("crates-proxy-label").click();
    // Wait for crates to refresh
    await this.waitForSearchResults();
  }

  /**
   * Check if the crates proxy is enabled.
   */
  async isCratesProxyEnabled(): Promise<boolean> {
    // The input element holds the checked state
    return await this.cratesProxySwitch.isChecked();
  }

  /**
   * Wait for search results to load.
   */
  async waitForSearchResults(timeout: number = 10000): Promise<void> {
    // Wait for loading to finish
    try {
      await this.loadingIndicator.waitFor({ state: "hidden", timeout });
    } catch {
      // Loading might be too fast to catch
    }
    // Give Vue more time to update the DOM after state changes (e.g., proxy toggle)
    await this.page.waitForTimeout(500);
  }

  /**
   * Get the count of visible crate cards.
   */
  async getCrateCount(): Promise<number> {
    // Get all crate cards in the grid
    return await this.crateCards.count();
  }

  /**
   * Check if the empty state is visible.
   */
  async hasNoCrates(): Promise<boolean> {
    return await this.emptyState.isVisible();
  }

  /**
   * Check if loading indicator is visible.
   */
  async isLoading(): Promise<boolean> {
    return await this.loadingIndicator.isVisible();
  }

  /**
   * Get crate names from visible cards.
   */
  async getCrateNames(): Promise<string[]> {
    const cards = this.crateCards;
    const count = await cards.count();
    const names: string[] = [];

    for (let i = 0; i < count; i++) {
      const card = cards.nth(i);
      const title = await card.getByTestId("crate-card-name").textContent();
      if (title) {
        names.push(title.trim());
      }
    }

    return names;
  }

  /**
   * Click on a crate card by name.
   */
  async clickCrate(crateName: string): Promise<void> {
    const card = this.crateCards.filter({
      hasText: crateName,
    });
    await card.click();
  }

  /**
   * Check if a specific crate is visible in the list.
   */
  async hasCrate(crateName: string): Promise<boolean> {
    const card = this.crateCards.filter({
      hasText: crateName,
    });
    return await card.isVisible();
  }

  /**
   * Scroll to load more crates (infinite scroll).
   */
  async scrollToLoadMore(): Promise<void> {
    const container = this.page.getByTestId("crates-scroll-container");
    await container.evaluate((el) => {
      el.scrollTop = el.scrollHeight;
    });
    // Wait for potential loading
    await this.page.waitForTimeout(500);
  }

  /**
   * Check if all crates have been loaded.
   */
  async isEndOfResults(): Promise<boolean> {
    return await this.endOfResults.isVisible();
  }

  /**
   * Get the current search text.
   */
  async getSearchText(): Promise<string> {
    return (await this.searchInput.inputValue()) ?? "";
  }
}
