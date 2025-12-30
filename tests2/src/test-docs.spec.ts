import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import {
  assertDockerAvailable,
  createBufferedTestLogger,
  ensureLocalKellnrTestImage,
  publishCrate,
  restrictToSingleWorkerBecauseFixedPorts,
  waitForHttpOk,
} from "./testUtils";
import { startContainer, withStartedContainer } from "./lib/docker";

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

function extractRegistryTokenFromCrateConfig(crateDir: string): string {
  const configPath = path.resolve(crateDir, ".cargo", "config.toml");
  const contents = fs.readFileSync(configPath, "utf8");

  const tokenMatch = contents.match(
    /kellnr-test\s*=\s*\{[^}]*token\s*=\s*"([^"]+)"[^}]*\}/,
  );
  if (!tokenMatch) {
    throw new Error(`Failed to extract kellnr-test token from ${configPath}`);
  }
  return tokenMatch[1];
}

test.describe("docs generation smoke test", () => {
  // Lua-test-style setup: Kellnr runs on stable localhost:8000, and crate-local `.cargo/config.toml`
  // can remain static.
  restrictToSingleWorkerBecauseFixedPorts();

  test("generates docs for published crate", async ({}, testInfo) => {
    testInfo.setTimeout(15 * 60 * 1000);

    const tlog = createBufferedTestLogger(testInfo, "test-docs");
    const log = tlog.log;

    // Unique container + data dir per worker/test
    const suffix = `${testInfo.workerIndex}-${Date.now()}`;
    const containerBaseName = `kellnr-docs-${suffix}`;

    const image = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";
    const registry = "kellnr-test";

    // Use fixed localhost:8000 so Kellnr generates stable URLs.
    const hostPort = 8000;
    const baseUrl = `http://localhost:${hostPort}`;
    const url = baseUrl;

    // Per-test data directory to mount as /opt/kdata
    const dataDir = path.resolve(
      process.cwd(),
      "tmp",
      "test-docs",
      `data-${suffix}`,
    );
    const dataDirInContainer = "/opt/kdata/";

    // Crate paths under tests2/
    const fullTomlCrateDir = path.resolve(
      process.cwd(),
      "crates",
      "test-docs",
      "full-toml",
    );

    // Expected docs path on the host (same structure as Lua test)
    const expectedDocsPath = path.resolve(
      dataDir,
      "docs",
      "full-toml",
      "1.0.0",
      "doc",
      "full_toml",
      "index.html",
    );

    try {
      await test.step("check prerequisites", async () => {
        await assertDockerAvailable();
        log("Docker is available");
      });

      await test.step("ensure Kellnr test image exists (globalSetup should have done this)", async () => {
        log(`Using image: ${image}`);
        await ensureLocalKellnrTestImage(image);
        log(`Image ready: ${image}`);
      });

      await test.step("prepare data directory", async () => {
        log(`Preparing data directory: ${dataDir}`);
        rimrafSync(dataDir);
        fs.mkdirSync(dataDir, { recursive: true });
      });

      // Extract token from crate config (same approach used in other ported tests)
      const registryToken =
        extractRegistryTokenFromCrateConfig(fullTomlCrateDir);

      const started = await startContainer(
        {
          name: containerBaseName,
          image,
          ports: { 8000: hostPort },
          env: {
            KELLNR_LOG__LEVEL: "trace",
            KELLNR_LOG__LEVEL_WEB_SERVER: "debug",
            KELLNR_DOCS__ENABLED: "true",

            // Ensure Kellnr generates URLs with localhost:8000
            KELLNR_ORIGIN__PORT: String(hostPort),
          },
          bindMounts: {
            [dataDir]: dataDirInContainer,
          },
        },
        testInfo,
      );

      await withStartedContainer(
        testInfo,
        started,
        async () => {
          await test.step("wait for server readiness", async () => {
            log(`Waiting for HTTP 200 on ${url}`);
            await waitForHttpOk(url, { timeoutMs: 60_000, intervalMs: 1_000 });
            log("Server ready");
          });

          await test.step("publish full-toml crate", async () => {
            log("Publishing crate: full-toml");

            await publishCrate({
              cratePath: "tests2/crates/test-docs/full-toml",
              registry,
              registryToken,
            });
          });

          await test.step("wait for docs to be generated", async () => {
            log(`Waiting for docs file: ${expectedDocsPath}`);
            await waitForFile(expectedDocsPath, {
              attempts: 60,
              delayMs: 2_000,
              log,
            });
            log("Docs generated");
          });

          await test.step("verify docs file content", async () => {
            const contents = fs.readFileSync(expectedDocsPath, "utf8");
            expect(contents).toContain("full_toml");
          });
        },
        { alwaysCollectLogs: true },
      );
    } finally {
      // Always remove per-test data directory to avoid accumulation.
      try {
        rimrafSync(dataDir);
      } catch {
        // best-effort
      }

      await tlog.flush();
    }
  });
});
