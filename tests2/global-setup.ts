import type { FullConfig } from "@playwright/test";
import path from "node:path";
import process from "node:process";
import { execa } from "execa";

async function execOrThrow(
  cmd: string,
  args: string[],
  opts?: { cwd?: string },
) {
  const res = await execa(cmd, args, {
    cwd: opts?.cwd,
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

function repoRootFromTests2(): string {
  // global-setup.ts lives in tests2/. When invoked by Playwright, process.cwd()
  // is typically tests2/, but we keep this robust.
  return path.resolve(process.cwd(), "..");
}

/**
 * Playwright globalSetup:
 * - Build the Kellnr test docker image once per *entire* test run (not per worker).
 * - Mirrors the Lua runner behavior that builds `kellnr-test:local` using `tests/Dockerfile`
 *   with repo root as build context.
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

  const repoRoot = repoRootFromTests2();

  // Build using the *repo root* as Docker build context.
  // The Dockerfile lives in tests2/ but must see the whole repo (justfile, crates, config, etc.)
  // to build the Kellnr binary. The Dockerfile itself copies the CA cert from `tests2/ca.crt`
  // so tests2 can become self-contained and `tests/` can be removed later.
  const dockerfile = path.resolve(repoRoot, "tests2", "Dockerfile");

  // eslint-disable-next-line no-console
  console.log(`[globalSetup] Building Docker image: ${image}`);
  // Equivalent to: docker build -f tests2/Dockerfile -t <image> --build-arg KELLNR_VERSION=local <repoRoot>
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
    { cwd: repoRoot },
  );

  // eslint-disable-next-line no-console
  console.log(`[globalSetup] Built Docker image: ${image}`);
}
