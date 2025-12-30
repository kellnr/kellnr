import { test } from "@playwright/test";
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

test.describe("auth required smoke test", () => {
  test("starts kellnr with auth required and can publish crates", async ({}, testInfo) => {
    testInfo.setTimeout(10 * 60 * 1000);

    const tlog = createBufferedTestLogger(testInfo, "test-auth-req");
    const log = tlog.log;

    try {
      await test.step("check prerequisites", async () => {
        await assertDockerAvailable();
        log("Docker is available");
      });

      const image = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";
      const container = `kellnr-auth-req-${testInfo.workerIndex}`;
      const registry = "kellnr-test";

      const hostPort = await allocateFreeLocalhostPort();
      const baseUrl = `http://localhost:${hostPort}`;
      const url = baseUrl;

      const crateCargoConfigPath = path.resolve(
        process.cwd(),
        "crates",
        "test-auth-req",
        "foo-bar",
        ".cargo",
        "config.toml",
      );
      const crateCargoConfig = fs.readFileSync(crateCargoConfigPath, "utf8");
      const tokenMatch = crateCargoConfig.match(
        /kellnr-test\s*=\s*\{[^}]*token\s*=\s*"([^"]+)"[^}]*\}/,
      );
      if (!tokenMatch) {
        throw new Error(
          `Failed to extract kellnr-test token from ${crateCargoConfigPath}`,
        );
      }
      const registryToken = tokenMatch[1];

      await test.step("ensure Kellnr test image exists (build if missing)", async () => {
        log(`Using image: ${image}`);
        await ensureLocalKellnrTestImage(image);
        log(`Image ready: ${image}`);
      });

      await withDockerContainer(testInfo, container, async () => {
        await test.step("start Kellnr container (auth required)", async () => {
          log(`Starting container: ${container}`);
          log(`Mapping host port ${hostPort} -> container 8000`);
          await dockerRun({
            name: container,
            image,
            ports: { [hostPort]: 8000 },
            env: {
              KELLNR_LOG__LEVEL: "debug",
              KELLNR_LOG__LEVEL_WEB_SERVER: "debug",
              KELLNR_REGISTRY__AUTH_REQUIRED: "true",
              KELLNR_ORIGIN__PORT: String(hostPort),
            },
          });
          log(`Container started: ${container}`);
        });

        await test.step("wait for server readiness", async () => {
          log(`Waiting for HTTP 200 on ${url}`);
          await waitForHttpOk(url, { timeoutMs: 60_000, intervalMs: 1_000 });
          log("Server ready");
        });

        await test.step("publish crates", async () => {
          log("Publishing crate: test_lib");
          await publishCrate({
            cratePath: "tests2/crates/test-auth-req/test_lib",
            registry,
            registryBaseUrl: baseUrl,
            registryToken,
          });

          log("Publishing crate: foo-bar");
          await publishCrate({
            cratePath: "tests2/crates/test-auth-req/foo-bar",
            registry,
            registryBaseUrl: baseUrl,
            registryToken,
          });

          log("Crate publishing finished");
        });

        await test.step("collect logs", async () => {
          log("Attaching docker logs");
          await writeDockerLogsArtifact(testInfo, container, "kellnr-auth-req");
          log("Docker logs attached");
        });
      });

      log("Done");
    } finally {
      await tlog.flush();
    }
  });
});
