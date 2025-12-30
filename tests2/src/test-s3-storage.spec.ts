import { test } from "@playwright/test";
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
import {
  attachContainerLogs,
  buildS3MinioImage,
  createNetwork,
  startContainer,
  startS3MinioContainer,
  withStartedContainer,
  withStartedNetwork,
} from "./lib/docker";

test.describe("s3 storage smoke test", () => {
  // Match the Lua test setup:
  // - Kellnr binds to localhost:8000
  // - crate-local `.cargo/config.toml` can stay static (hardcoded localhost:8000)
  // - crates.io proxy download URLs are stable
  restrictToSingleWorkerBecauseFixedPorts();

  test("starts kellnr with S3 enabled (minio) and can publish crates", async ({}, testInfo) => {
    testInfo.setTimeout(15 * 60 * 1000);

    const tlog = createBufferedTestLogger(testInfo, "test-s3-storage");
    const log = tlog.log;

    // Keep names unique across parallel workers
    const suffix = `${testInfo.workerIndex}-${Date.now()}`;
    const networkBaseName = `s3-net-${suffix}`;
    const minioBaseName = `minio-${suffix}`;
    const kellnrBaseName = `kellnr-s3-${suffix}`;

    const kellnrImage = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";
    const registry = "kellnr-test";

    // S3 settings (mirror Lua test)
    const s3RootUser = "minioadmin";
    const s3RootPassword = "minioadmin";
    const s3UrlInDockerNet = "http://minio:9000";
    const s3AllowHttp = "true";
    const s3Image = `custom-minio-${suffix}`;
    const s3CratesBucket = "kellnr-crates";
    const s3CratesioBucket = "kellnr-cratesio";

    // Use fixed localhost:8000 so Kellnr generates stable URLs.
    const hostPort = 8000;
    const baseUrl = `http://localhost:${hostPort}`;
    const url = baseUrl;

    // Extract the registry token from the crate config (same token is used across these test crates)
    const crateCargoConfigPath = path.resolve(
      process.cwd(),
      "crates",
      "test-s3-storage",
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

    try {
      await test.step("check prerequisites", async () => {
        await assertDockerAvailable();
        log("Docker is available");
      });

      await test.step("ensure Kellnr test image exists (build if missing)", async () => {
        log(`Using image: ${kellnrImage}`);
        await ensureLocalKellnrTestImage(kellnrImage);
        log(`Image ready: ${kellnrImage}`);
      });

      const startedNetwork = await createNetwork(networkBaseName, testInfo);

      await withStartedNetwork(startedNetwork, async (network) => {
        await test.step("build minio image", async () => {
          log(`Building minio image: ${s3Image}`);

          await buildS3MinioImage({
            imageName: s3Image,
            cratesBucket: s3CratesBucket,
            cratesioBucket: s3CratesioBucket,
          });

          log(`Minio image built: ${s3Image}`);
        });

        const startedMinio = await startS3MinioContainer(
          {
            name: minioBaseName,
            image: s3Image,
            network,
            rootUser: s3RootUser,
            rootPassword: s3RootPassword,
          },
          testInfo,
        );

        await withStartedContainer(
          testInfo,
          startedMinio,
          async () => {
            log("Minio container started");

            const startedKellnr = await startContainer(
              {
                name: kellnrBaseName,
                image: kellnrImage,
                network,
                // Fixed port mapping (Lua-style): host 8000 -> container 8000
                ports: { 8000: hostPort },
                env: {
                  KELLNR_LOG__LEVEL: "debug",
                  KELLNR_LOG__LEVEL_WEB_SERVER: "debug",
                  KELLNR_PROXY__ENABLED: "true",

                  KELLNR_S3__ENABLED: "true",
                  KELLNR_S3__ACCESS_KEY: s3RootUser,
                  KELLNR_S3__SECRET_KEY: s3RootPassword,
                  KELLNR_S3__ENDPOINT: s3UrlInDockerNet,
                  KELLNR_S3__ALLOW_HTTP: s3AllowHttp,
                  KELLNR_S3__CRATES_BUCKET: s3CratesBucket,
                  KELLNR_S3__CRATESIO_BUCKET: s3CratesioBucket,

                  // Ensure Kellnr generates URLs with localhost:8000 (cratesio proxy download URLs)
                  KELLNR_ORIGIN__PORT: String(hostPort),
                },
              },
              testInfo,
            );

            await withStartedContainer(
              testInfo,
              startedKellnr,
              async () => {
                await test.step("wait for server readiness", async () => {
                  log(`Waiting for HTTP 200 on ${url}`);
                  await waitForHttpOk(url, {
                    timeoutMs: 60_000,
                    intervalMs: 1_000,
                  });
                  log("Server ready");
                });

                await test.step("publish crates", async () => {
                  log("Publishing crate: test_lib");
                  await publishCrate({
                    cratePath: "tests2/crates/test-s3-storage/test_lib",
                    registry,
                    registryToken,
                  });

                  log("Publishing crate: UpperCase-Name123");
                  await publishCrate({
                    cratePath:
                      "tests2/crates/test-s3-storage/UpperCase-Name123",
                    registry,
                    registryToken,
                  });

                  log("Publishing crate: foo-bar");
                  await publishCrate({
                    cratePath: "tests2/crates/test-s3-storage/foo-bar",
                    registry,
                    registryToken,
                  });

                  log("Crate publishing finished");
                });

                await test.step("collect logs", async () => {
                  log("Attaching docker logs");
                  await attachContainerLogs(testInfo, startedKellnr.container, {
                    name: "kellnr-s3",
                  });
                  await attachContainerLogs(testInfo, startedMinio.container, {
                    name: "minio",
                  });
                  log("Docker logs attached");
                });
              },
              { alwaysCollectLogs: true },
            );
          },
          { alwaysCollectLogs: true },
        );
      });
    } finally {
      await tlog.flush();
    }
  });
});
