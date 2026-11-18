import { expect, type Page, type Locator } from "@playwright/test";
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
   * Wait until the first page of results is shown.
   */
  async waitForPageLoad(): Promise<void> {
    await this.waitForSearchResults();
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
   * Navigate to the list of cached crates.io crates.
   */
  async gotoCachedCrates(): Promise<void> {
    await super.goto("/crates?proxy=1");
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
    const nextProxyMode = !(await this.cratesProxySwitch.isChecked());
    const results = this.page.waitForResponse((response) => {
      const url = new URL(response.url());
      return response.ok() &&
        ["/api/v1/ui/crates", "/api/v1/ui/search"].some((path) => url.pathname.endsWith(path)) &&
        url.searchParams.get("cache") === String(nextProxyMode) &&
        url.searchParams.get("page") === "0";
    });
    // Click the visible switch label, not the hidden input
    await this.page.getByTestId("crates-proxy-label").click();
    await results;
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
    const query = (await this.searchInput.inputValue()).trim();
    const resultCount = this.page.getByTestId("crates-result-count");
    if (query) {
      await expect(resultCount).toContainText(`found for "${query}"`, { timeout });
    } else {
      await expect(resultCount).toHaveText(/\bcrates?(?: with)?$/, { timeout });
    }
    await expect(this.loadingIndicator).toBeHidden({ timeout });
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
