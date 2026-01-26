/**
 * UI tests for documentation generation with KELLNR_ORIGIN__PATH prefix.
 *
 * Tests:
 * - Publishes a crate with docs enabled when Kellnr runs under a path prefix
 * - Waits for rustdoc to generate documentation
 * - Verifies docs files exist on disk
 * - Verifies docs link includes the path prefix and opens correctly
 *
 * This test verifies the fix for issue #983:
 * "Web UI 'Open documentation' link ignores KELLNR_ORIGIN__PATH"
 *
 * Performance: Uses a single local Kellnr instance for the test.
 */

import { test, expect } from "./lib/ui-fixtures";
import { CratePage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  assertKellnrBinaryExists,
  publishCrate,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";
import fs from "node:fs";
import path from "node:path";

function fileExists(p: string): boolean {
  try {
    return fs.existsSync(p);
  } catch {
    return false;
  }
}

async function waitForFile(
  filePath: string,
  opts: { attempts: number; delayMs: number; log?: (msg: string) => void },
): Promise<void> {
  for (let i = 1; i <= opts.attempts; i++) {
    if (fileExists(filePath)) return;
    opts.log?.(
      `Docs not found yet (attempt ${i}/${opts.attempts}): ${filePath}`,
    );
    await new Promise((r) => setTimeout(r, opts.delayMs));
  }
  throw new Error(`Docs file not generated in time: ${filePath}`);
}

test.describe("Documentation Generation UI Tests with Path Prefix", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;
  let expectedDocsPath: string;
  const pathPrefix = "/kellnr";

  test.beforeAll(async () => {
    // Docs generation needs more time
    test.setTimeout(15 * 60 * 1000); // 15 minutes for docs generation

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    const suffix = `${Date.now()}`;

    started = await startLocalKellnr({
      name: `kellnr-ui-docs-path-${suffix}`,
      env: {
        KELLNR_DOCS__ENABLED: "true",
        KELLNR_ORIGIN__PATH: pathPrefix,
      },
      healthCheckPath: pathPrefix,
    });

    // The base URL includes the path prefix for navigation
    baseUrl = `${started.baseUrl}${pathPrefix}`;

    // Expected docs path in the local Kellnr data directory
    expectedDocsPath = path.resolve(
      started.dataDir,
      "docs",
      "full-toml",
      "1.0.0",
      "doc",
      "full_toml",
      "index.html",
    );

    console.log(`[setup] Server ready at ${baseUrl}`);
    console.log(`[setup] Path prefix: ${pathPrefix}`);
    console.log(`[setup] Data directory: ${started.dataDir}`);

    // Publish crate with docs (using path-prefixed registry config)
    const registry = "kellnr-test-path";
    const fullTomlCrateDir = path.resolve(
      process.cwd(),
      "crates",
      "test-docs",
      "full-toml-path-prefix",
    );

    const registryToken = extractRegistryTokenFromCargoConfig({
      crateDir: fullTomlCrateDir,
      registryName: registry,
    });

    console.log("[setup] Publishing crate: full-toml (with path prefix)");
    await publishCrate({
      cratePath: "tests/crates/test-docs/full-toml-path-prefix",
      registry,
      registryToken,
    });

    console.log("[setup] Waiting for docs to be generated");
    await waitForFile(expectedDocsPath, {
      attempts: 60,
      delayMs: 2_000,
      log: console.log,
    });
    console.log("[setup] Docs generated");
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
    }
  });

  test("docs file exists on disk", async () => {
    // Verify the docs file was generated
    const contents = fs.readFileSync(expectedDocsPath, "utf8");
    expect(contents).toContain("full_toml");
  });

  test("docs link includes path prefix and opens docs in new tab", async ({
    page,
    context,
  }) => {
    // Docs are confirmed to exist on disk (first test passed)
    // Background job should have indexed them by now

    // Navigate to the crate page using the path prefix
    await page.goto(`${baseUrl}/crate?name=full-toml`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    // Wait for the "Open documentation" link to appear
    // (it's a clickable div, not a button)
    await cratePage.openDocsButton.waitFor({ state: "visible", timeout: 30000 });

    // Click the link and verify a new page opens with docs
    const [newPage] = await Promise.all([
      context.waitForEvent("page"),
      cratePage.openDocsButton.click(),
    ]);

    // Verify the new page URL contains BOTH the path prefix AND the docs path
    await newPage.waitForLoadState();
    const docsUrl = newPage.url();

    // The docs URL should include the path prefix (this is the fix for #983)
    expect(docsUrl).toContain(`${pathPrefix}/docs/full-toml/1.0.0`);

    // Verify the docs page has content
    const docsContent = await newPage.content();
    expect(docsContent).toContain("full_toml");

    // Close the new page
    await newPage.close();
  });
});
