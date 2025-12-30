import { test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import {
  allocateFreeLocalhostPort,
  assertDockerAvailable,
  createBufferedTestLogger,
  dockerBuild,
  dockerNetworkCreate,
  dockerNetworkRemove,
  dockerRun,
  dockerStop,
  ensureLocalKellnrTestImage,
  publishCrate,
  waitForHttpOk,
  withDockerContainer,
  writeDockerLogsArtifact,
} from "./testUtils";

test.describe("s3 storage smoke test", () => {
  test("starts kellnr with S3 enabled (minio) and can publish crates", async ({}, testInfo) => {
    testInfo.setTimeout(15 * 60 * 1000);

    const tlog = createBufferedTestLogger(testInfo, "test-s3-storage");
    const log = tlog.log;

    // Keep names unique across parallel workers
    const suffix = `${testInfo.workerIndex}-${Date.now()}`;
    const network = `s3-net-${suffix}`;
    const minioContainer = `minio-${suffix}`;
    const kellnrContainer = `kellnr-s3-${suffix}`;

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

    // Kellnr will run on a dynamic host port; tell Kellnr the external port so it generates correct config.json
    const hostPort = await allocateFreeLocalhostPort();
    const baseUrl = `http://localhost:${hostPort}`;
    const url = baseUrl;

    // Extract the registry token from the crate config (same token is used across these test crates)
    const crateCargoConfigPath = path.resolve(
      process.cwd(),
      "test-s3-storage",
      "crates",
      "foo-bar",
      ".cargo",
      "config.toml",
    );
    const crateCargoConfig = fs.readFileSync(crateCargoConfigPath, "utf8");
    const tokenMatch = crateCargoConfig.match(
      /kellnr-test\s*=\s*\{[^}]*token\s*=\s*"([^"]+)"[^}]*\}/,
    );
    if (!tokenMatch) {
      throw new Error(`Failed to extract kellnr-test token from ${crateCargoConfigPath}`);
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

      await test.step("create docker network", async () => {
        log(`Creating docker network: ${network}`);
        await dockerNetworkCreate(network);
        log(`Docker network created: ${network}`);
      });

      // Make sure network is removed even if the test fails early
      try {
        await test.step("build minio image", async () => {
          log(`Building minio image: ${s3Image}`);

          // Build context is the original Lua test folder (tests/test-s3-storage)
          // Dockerfile is tests/test-s3-storage/Dockerfile
          const repoRoot = path.resolve(process.cwd(), "..");
          const dockerfile = path.resolve(repoRoot, "tests", "test-s3-storage", "Dockerfile");
          const contextDir = path.resolve(repoRoot, "tests", "test-s3-storage");

          await dockerBuild({
            tag: s3Image,
            dockerfile,
            contextDir,
            buildArgs: {
              CRATES_BUCKET: s3CratesBucket,
              CRATESIO_BUCKET: s3CratesioBucket,
            },
          });

          log(`Minio image built: ${s3Image}`);
        });

        await test.step("start minio container", async () => {
          // We do not need host port mapping for minio; Kellnr reaches it via docker network alias "minio".
          log(`Starting minio container: ${minioContainer} (network=${network})`);
          await dockerRun({
            name: minioContainer,
            image: s3Image,
            env: {
              MINIO_ROOT_USER: s3RootUser,
              MINIO_ROOT_PASSWORD: s3RootPassword,
            },
            extraArgs: ["--network", network, "--network-alias", "minio"],
          });
          log(`Minio container started: ${minioContainer}`);
        });

        await withDockerContainer(testInfo, kellnrContainer, async () => {
          await test.step("start kellnr container (S3 enabled)", async () => {
            log(`Starting container: ${kellnrContainer}`);
            log(`Mapping host port ${hostPort} -> container 8000 (KELLNR_ORIGIN__PORT=${hostPort})`);

            await dockerRun({
              name: kellnrContainer,
              image: kellnrImage,
              ports: { [hostPort]: 8000 },
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

                // Required for correct sparse index config.json URLs
                KELLNR_ORIGIN__PORT: String(hostPort),
              },
              extraArgs: ["--network", network],
            });

            log(`Container started: ${kellnrContainer}`);
          });

          await test.step("wait for server readiness", async () => {
            log(`Waiting for HTTP 200 on ${url}`);
            await waitForHttpOk(url, { timeoutMs: 60_000, intervalMs: 1_000 });
            log("Server ready");
          });

          await test.step("publish crates", async () => {
            log("Publishing crate: test_lib");
            await publishCrate({
              cratePath: "tests2/test-s3-storage/crates/test_lib",
              registry,
              registryBaseUrl: baseUrl,
              registryToken,
            });

            log("Publishing crate: UpperCase-Name123");
            await publishCrate({
              cratePath: "tests2/test-s3-storage/crates/UpperCase-Name123",
              registry,
              registryBaseUrl: baseUrl,
              registryToken,
            });

            log("Publishing crate: foo-bar");
            await publishCrate({
              cratePath: "tests2/test-s3-storage/crates/foo-bar",
              registry,
              registryBaseUrl: baseUrl,
              registryToken,
            });

            log("Crate publishing finished");
          });

          await test.step("collect logs", async () => {
            log("Attaching docker logs");
            await writeDockerLogsArtifact(testInfo, kellnrContainer, "kellnr-s3");
            await writeDockerLogsArtifact(testInfo, minioContainer, "minio");
            log("Docker logs attached");
          });
        });

        log("Done");
      } finally {
        await test.step("cleanup minio + network", async () => {
          log("Cleaning up minio container and docker network");
          await dockerStop(minioContainer);
          await dockerNetworkRemove(network);
          log("Cleanup finished");
        });
      }
    } finally {
      await tlog.flush();
    }
  });
});
