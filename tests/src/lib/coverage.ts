/**
 * Coverage collection utilities for Playwright tests.
 *
 * Uses V8 coverage (Chromium only) to collect JavaScript coverage
 * during E2E test execution.
 */

import type { Page } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const COVERAGE_DIR = path.join(__dirname, "../../coverage");

/**
 * Check if coverage collection is enabled via COVERAGE environment variable.
 */
export function isCoverageEnabled(): boolean {
  return !!process.env.COVERAGE;
}

/**
 * Check if the browser is Chromium (V8 coverage only works in Chromium).
 */
export function isChromium(page: Page): boolean {
  return page.context().browser()?.browserType().name() === "chromium";
}

/**
 * Start JavaScript coverage collection on a page.
 * Only collects coverage if COVERAGE env var is set and browser is Chromium.
 */
export async function startCoverage(page: Page): Promise<void> {
  if (!isCoverageEnabled() || !isChromium(page)) {
    return;
  }

  await page.coverage.startJSCoverage({
    resetOnNavigation: false,
  });
}

/**
 * Stop coverage collection and save the results to a JSON file.
 * @param page - The Playwright page
 * @param testName - Name of the test (used for the output filename)
 */
export async function stopCoverage(page: Page, testName: string): Promise<void> {
  if (!isCoverageEnabled() || !isChromium(page)) {
    return;
  }

  const coverage = await page.coverage.stopJSCoverage();

  if (!fs.existsSync(COVERAGE_DIR)) {
    fs.mkdirSync(COVERAGE_DIR, { recursive: true });
  }

  // Save raw V8 coverage as JSON
  const safeName = testName.replace(/[^a-z0-9]/gi, "_");
  const coverageFile = path.join(COVERAGE_DIR, `${safeName}.json`);
  fs.writeFileSync(coverageFile, JSON.stringify(coverage, null, 2));
}
