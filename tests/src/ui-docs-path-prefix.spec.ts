/**
 * UI tests for documentation links with KELLNR_ORIGIN__PATH prefix.
 *
 * Tests:
 * - Publishes a crate under a path prefix and uploads a small docs archive
 * - Verifies the uploaded docs file exists on disk
 * - Verifies docs link includes the path prefix and opens correctly
 *
 * The normal-path suite tests automatic rustdoc generation. This suite checks
 * the path-prefix link without running the expensive generator a second time.
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

test.describe("Documentation UI Tests with Path Prefix", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;
  let expectedDocsPath: string;
  const pathPrefix = "/kellnr";

  test.beforeAll(async () => {
    // Publishing the fixture can still be slow with an empty Cargo cache.
    test.setTimeout(15 * 60 * 1000);

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    const suffix = `${Date.now()}`;

    started = await startLocalKellnr({
      name: `kellnr-ui-docs-path-${suffix}`,
      env: {
        // The normal-path suite covers auto-generation; don't queue rustdoc here.
        KELLNR_DOCS__ENABLED: "false",
        KELLNR_ORIGIN__PATH: pathPrefix,
      },
      healthCheckPath: `${pathPrefix}/api/v1/health`,
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

    // Publish the crate using the path-prefixed registry config.
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

    // Manual upload exercises the same docs URL generation as the auto-builder
    // without compiling the crate's dependencies a second time.
    // The tiny ZIP contains doc/full_toml/index.html.
    const archivePath = path.resolve(process.cwd(), "fixtures", "test-docs", "full-toml.zip");
    const response = await fetch(`${baseUrl}/api/v1/docs/full-toml/1.0.0`, {
      method: "PUT",
      headers: {
        Authorization: registryToken,
        "Content-Type": "application/octet-stream",
      },
      body: fs.readFileSync(archivePath),
    });
    const responseBody = await response.text();
    expect(response.status, responseBody).toBe(200);
    expect(JSON.parse(responseBody).url).toBe(`${pathPrefix}/docs/full-toml/1.0.0/doc/full_toml/index.html`);
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
    }
  });

  test("docs file exists on disk", async () => {
    const contents = fs.readFileSync(expectedDocsPath, "utf8");
    expect(contents).toContain("full_toml");
  });

  test("docs link includes path prefix and opens docs in new tab", async ({
    page,
    context,
  }) => {
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

    // The docs URL should include the path prefix.
    expect(docsUrl).toContain(`${pathPrefix}/docs/full-toml/1.0.0`);

    // Verify the docs page has content
    const docsContent = await newPage.content();
    expect(docsContent).toContain("full_toml");

    // Close the new page
    await newPage.close();
  });
});
