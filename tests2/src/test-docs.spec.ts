import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import {
  allocateFreeLocalhostPort,
  assertDockerAvailable,
  createBufferedTestLogger,
  dockerRun,
  ensureLocalKellnrTestImage,
  publishCrate,
  waitForHttpOk,
  withDockerContainer,
  writeDockerLogsArtifact,
} from "./testUtils";

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
  test("generates docs for published crate", async ({}, testInfo) => {
    testInfo.setTimeout(15 * 60 * 1000);

    const tlog = createBufferedTestLogger(testInfo, "test-docs");
    const log = tlog.log;

    // Unique container + data dir per worker/test
    const suffix = `${testInfo.workerIndex}-${Date.now()}`;
    const container = `kellnr-docs-${suffix}`;

    const image = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";
    const registry = "kellnr-test";

    // Dynamic port per test to allow parallel execution
    const hostPort = await allocateFreeLocalhostPort();
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

      await withDockerContainer(testInfo, container, async () => {
        await test.step("start Kellnr container (docs enabled)", async () => {
          log(`Starting container: ${container}`);
          log(`Mapping host port ${hostPort} -> container 8000`);
          log(`Mounting data dir: ${dataDir} -> ${dataDirInContainer}`);

          await dockerRun({
            name: container,
            image,
            ports: { [hostPort]: 8000 },
            env: {
              KELLNR_LOG__LEVEL: "trace",
              KELLNR_LOG__LEVEL_WEB_SERVER: "debug",
              KELLNR_DOCS__ENABLED: "true",

              // Required for correct sparse index config.json URLs
              KELLNR_ORIGIN__PORT: String(hostPort),
            },
            extraArgs: ["-v", `${dataDir}:${dataDirInContainer}`],
          });

          log(`Container started: ${container}`);
        });

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
            registryBaseUrl: baseUrl,
            registryToken,
            overrideCratesIo: false,
          });

          log("Crate publish finished");
        });

        await test.step("verify docs generated", async () => {
          log(`Waiting for docs file: ${expectedDocsPath}`);

          await waitForFile(expectedDocsPath, {
            attempts: 30,
            delayMs: 2_000,
            log,
          });

          expect(fileExists(expectedDocsPath)).toBeTruthy();
          log("Docs generated successfully");
        });

        await test.step("collect logs", async () => {
          log("Attaching docker logs");
          await writeDockerLogsArtifact(testInfo, container, "kellnr-docs");
          log("Docker logs attached");
        });

        log("Done");
      });
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
