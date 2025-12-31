import type { FullConfig } from "@playwright/test";
import path from "node:path";
import process from "node:process";
import { execa } from "execa";

async function execOrThrow(
  cmd: string,
  args: string[],
  opts?: { cwd?: string; env?: NodeJS.ProcessEnv },
) {
  const res = await execa(cmd, args, {
    cwd: opts?.cwd,
    env: opts?.env,
    stdio: "inherit",
  });
  if (res.exitCode !== 0) {
    throw new Error(
      `Command failed: ${cmd} ${args.join(" ")} (exitCode=${res.exitCode})`,
    );
  }
}

async function dockerImageExists(image: string): Promise<boolean> {
  try {
    const res = await execa("docker", ["image", "inspect", image], {
      stdio: "ignore",
    });
    return res.exitCode === 0;
  } catch {
    return false;
  }
}

function repoRootFromTests(): string {
  // global-setup.ts lives in tests/. When invoked by Playwright, process.cwd()
  // is typically tests/, but we keep this robust.
  return path.resolve(process.cwd(), "..");
}

/**
 * Playwright globalSetup:
 * - Build the Kellnr test docker image once per *entire* test run (not per worker).
 * - Builds `kellnr-test:local` using `tests/Dockerfile` with repo root as build context.
 */
export default async function globalSetup(_config: FullConfig) {
  const image = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";

  // Basic Docker availability check (fast, helpful error).
  await execOrThrow("docker", ["version"]);

  // If the image already exists locally, do nothing.
  if (await dockerImageExists(image)) {
    // eslint-disable-next-line no-console
    console.log(`[globalSetup] Docker image exists: ${image}`);
    return;
  }

  const repoRoot = repoRootFromTests();

  // Build using the *repo root* as Docker build context.
  // The Dockerfile lives in tests/ but must see the whole repo (justfile, crates, config, etc.)
  // to build the Kellnr binary. The Dockerfile itself copies the CA cert from `tests/ca.crt`.
  const dockerfile = path.resolve(repoRoot, "tests", "Dockerfile");

  // Enable BuildKit for cache mounts in the Dockerfile (RUN --mount=type=cache,...).
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    DOCKER_BUILDKIT: "1",
    COMPOSE_DOCKER_CLI_BUILD: "1",
  };

  // eslint-disable-next-line no-console
  console.log(`[globalSetup] Building Docker image: ${image}`);
  // Equivalent to:
  //   DOCKER_BUILDKIT=1 docker build -f tests/Dockerfile -t <image> --build-arg KELLNR_VERSION=local <repoRoot>
  await execOrThrow(
    "docker",
    [
      "build",
      "-f",
      dockerfile,
      "-t",
      image,
      "--build-arg",
      "KELLNR_VERSION=local",
      repoRoot,
    ],
    { cwd: repoRoot, env },
  );

  // eslint-disable-next-line no-console
  console.log(`[globalSetup] Built Docker image: ${image}`);
}
