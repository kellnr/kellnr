/**
 * UI tests for documentation generation.
 *
 * Tests:
 * - Publishes a crate with docs enabled
 * - Waits for rustdoc to generate documentation
 * - Verifies docs files exist on disk
 * - Verifies docs link is clickable in the UI and opens in new tab
 *
 * Performance: Uses a single Kellnr container for the test.
 */

import { test, expect } from "./lib/ui-fixtures";
import { CratePage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  waitForHttpOk,
  assertDockerAvailable,
  publishCrate,
} from "./testUtils";
import { startKellnr, type StartedKellnr } from "./lib/kellnr";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";
import fs from "node:fs";
import path from "node:path";

function rimrafSync(p: string) {
  fs.rmSync(p, { recursive: true, force: true });
}

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

test.describe("Documentation Generation UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedKellnr;
  let baseUrl: string;
  let dataDir: string;
  let expectedDocsPath: string;

  test.beforeAll(async ({}, testInfo) => {
    // Container setup needs more time than default timeout
    test.setTimeout(15 * 60 * 1000); // 15 minutes for docs generation

    await assertDockerAvailable();
    console.log("[setup] Docker is available");

    const image = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";
    const suffix = `${Date.now()}`;

    // Per-test data directory to mount as /opt/kdata
    dataDir = path.resolve(
      process.cwd(),
      "tmp",
      "ui-test-docs",
      `data-${suffix}`,
    );
    const dataDirInContainer = "/opt/kdata/";

    // Expected docs path on the host
    expectedDocsPath = path.resolve(
      dataDir,
      "docs",
      "full-toml",
      "1.0.0",
      "doc",
      "full_toml",
      "index.html",
    );

    console.log("[setup] Preparing data directory");
    rimrafSync(dataDir);
    fs.mkdirSync(dataDir, { recursive: true });

    started = await startKellnr(
      {
        name: `kellnr-ui-docs-${suffix}`,
        image,
        env: {
          KELLNR_DOCS__ENABLED: "true",
        },
        bindMounts: {
          [dataDir]: dataDirInContainer,
        },
      },
      testInfo
    );

    baseUrl = started.baseUrl;

    console.log(`[setup] Waiting for HTTP 200 on ${baseUrl}`);
    await waitForHttpOk(baseUrl, {
      timeoutMs: 60_000,
      intervalMs: 1_000,
    });
    console.log("[setup] Server ready");

    // Publish crate with docs
    const registry = "kellnr-test";
    const fullTomlCrateDir = path.resolve(
      process.cwd(),
      "crates",
      "test-docs",
      "full-toml",
    );

    const registryToken = extractRegistryTokenFromCargoConfig({
      crateDir: fullTomlCrateDir,
      registryName: registry,
    });

    console.log("[setup] Publishing crate: full-toml");
    await publishCrate({
      cratePath: "tests/crates/test-docs/full-toml",
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
      console.log("[teardown] Stopping container");
      await started.started.container.stop();
    }

    // Cleanup data directory
    try {
      rimrafSync(dataDir);
    } catch {
      // best-effort
    }
  });

  test("docs file exists on disk", async () => {
    // Verify the docs file was generated
    const contents = fs.readFileSync(expectedDocsPath, "utf8");
    expect(contents).toContain("full_toml");
  });

  test("docs link appears and opens docs in new tab", async ({ page, context }) => {
    // Docs are confirmed to exist on disk (first test passed)
    // Background job should have indexed them by now

    // Navigate to the crate page
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

    // Verify the new page URL contains the docs path
    await newPage.waitForLoadState();
    expect(newPage.url()).toContain("/docs/full-toml/1.0.0");

    // Verify the docs page has content
    const docsContent = await newPage.content();
    expect(docsContent).toContain("full_toml");

    // Close the new page
    await newPage.close();
  });
});
