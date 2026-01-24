import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { setTimeout as sleep } from "node:timers/promises";
import { execa, ExecaError, type Options as ExecaOptions } from "execa";
import { expect, test, type TestInfo } from "@playwright/test";

/**
 * Minimal TestInfo-like type for use in beforeAll hooks where full TestInfo is unavailable.
 * Provides the subset of properties needed by container startup helpers.
 */
export type BeforeAllTestInfo = Pick<TestInfo, "title"> & {
  workerIndex?: number;
};

/**
 * Create a minimal TestInfo-like object for use in beforeAll hooks.
 * This avoids `as any` casts while satisfying the container helpers' requirements.
 */
export function createBeforeAllTestInfo(title: string): BeforeAllTestInfo {
  return { title, workerIndex: 0 };
}

type LogLevel = "info" | "debug" | "error";

function nowIsoNoMs(): string {
  // Keep filenames stable-ish and readable.
  return new Date()
    .toISOString()
    .replace(/\.\d{3}Z$/, "Z")
    .replace(/[:]/g, "-");
}

function playDebug(): boolean {
  return (
    process.env.DEBUG?.includes("tests") || process.env.TESTS_DEBUG === "1"
  );
}

function log(level: LogLevel, msg: string): void {
  const prefix = level.toUpperCase().padEnd(5);
  // Keep output CI-friendly (no ANSI here; Playwright highlights errors anyway).
  // eslint-disable-next-line no-console
  console.log(`[${prefix}] ${msg}`);
}

export function ensureDir(dir: string): void {
  fs.mkdirSync(dir, { recursive: true });
}

export function repoRootFromTests(): string {
  // tests/ is at repoRoot/tests
  return path.resolve(process.cwd(), "..");
}

export function testsRoot(): string {
  return path.resolve(process.cwd());
}

export function logsDir(): string {
  return path.resolve(testsRoot(), "logs");
}

export function projectTmpDir(testInfo: TestInfo): string {
  // Per-test directory for ephemeral artifacts (docker logs, etc.).
  const dir = path.resolve(
    testsRoot(),
    "tmp",
    `${testInfo.title.replace(/[^\w.-]+/g, "_")}-${testInfo.workerIndex}`,
  );
  ensureDir(dir);
  return dir;
}

/**
 * Buffered per-test logger:
 * - Captures log lines during the test
 * - Prints them as one block at the end to avoid interleaved output when running workers in parallel
 *
 * Usage:
 *   const tlog = createBufferedTestLogger(testInfo, "test-auth-req");
 *   tlog.log("Starting container...");
 *   ...
 *   await tlog.flush();
 */
export function createBufferedTestLogger(
  testInfo: TestInfo,
  prefix: string,
): {
  log: (msg: string) => void;
  flush: () => Promise<void>;
  attach: () => Promise<void>;
} {
  const lines: string[] = [];
  const startedAt = Date.now();

  const logLine = (msg: string) => {
    const line = `[${prefix}] ${msg}`;
    lines.push(line);
  };

  const attach = async () => {
    const content =
      lines.length > 0
        ? lines.join("\n") + "\n"
        : `[${prefix}] (no log lines captured)\n`;

    await testInfo.attach(`${prefix}-log`, {
      body: Buffer.from(content, "utf8"),
      contentType: "text/plain",
    });
  };

  const flush = async () => {
    const durationMs = Date.now() - startedAt;
    const header = `----- ${prefix} (worker=${testInfo.workerIndex}, duration=${durationMs}ms) -----`;
    const footer = `----- /${prefix} -----`;

    // Print as a single block to reduce interleaving between workers.
    // eslint-disable-next-line no-console
    console.log(
      [header, ...lines, footer].filter(Boolean).join("\n") +
      (lines.length ? "\n" : "\n"),
    );


    // Always attach as well (useful in CI artifacts / HTML report).
    await attach();
  };

  return { log: logLine, flush, attach };
}

export type ExecResult = {
  cmd: string;
  exitCode: number;
  stdout: string;
  stderr: string;
};

export async function exec(
  cmd: string,
  args: string[] = [],
  options: ExecaOptions = {},
): Promise<ExecResult> {
  const full = [cmd, ...args].join(" ");
  if (playDebug()) log("debug", `exec: ${full}`);

  try {
    const subprocess = await execa(cmd, args, {
      stdio: "pipe",
      ...options,
    });

    return {
      cmd: full,
      exitCode: subprocess.exitCode ?? 0,
      stdout: subprocess.stdout ?? "",
      stderr: subprocess.stderr ?? "",
    };
  } catch (e) {
    const err = e as ExecaError;

    const stdout = typeof err.stdout === "string" ? err.stdout : "";
    const stderr = typeof err.stderr === "string" ? err.stderr : "";

    return {
      cmd: full,
      exitCode: typeof err.exitCode === "number" ? err.exitCode : 1,
      stdout,
      stderr,
    };
  }
}

export async function execOrThrow(
  cmd: string,
  args: string[] = [],
  options: ExecaOptions = {},
  context?: { description?: string },
): Promise<ExecResult> {
  const res = await exec(cmd, args, options);
  if (res.exitCode !== 0) {
    const desc = context?.description ? ` (${context.description})` : "";
    const combined = [res.stdout, res.stderr].filter(Boolean).join("\n");

    throw new Error(
      `Command failed${desc}: ${res.cmd}\n` +
      `exitCode=${res.exitCode}\n` +
      (combined ? `output:\n${combined}\n` : ""),
    );

  }
  return res;
}

/**
 * Docker helpers have been moved to a central library that uses `testcontainers`
 * (see `src/lib/docker.ts`).
 *
 * Keep this module focused on non-docker concerns (logging, HTTP waits, cargo publish, etc.).
 *
 * The following legacy Docker CLI helpers were intentionally removed:
 * - dockerBuild / dockerPull / dockerNetworkCreate / dockerNetworkRemove
 * - dockerRun / dockerStop / dockerLogs / writeDockerLogsArtifact
 * - withDockerContainer
 *
 * Specs should now use idiomatic `testcontainers` container/network objects directly,
 * with shared helpers living in the central docker library.
 */

/**
 * NOTE:
 * Docker image existence checks were previously implemented via `docker image inspect`.
 * With the switch to `testcontainers`, the recommended approach is:
 * - keep building the shared integration image in Playwright `global-setup.ts`
 * - let container start failures surface as test errors (they’ll include Docker daemon logs)
 *
 * If you still want an explicit "ensure image exists" check, implement it in the central
 * docker library using the Docker API/testcontainers internals so tests don't shell out.
 */
export async function ensureLocalKellnrTestImage(
  _image: string,
): Promise<void> {
  // Intentionally a no-op now. globalSetup owns image creation.
  // Keeping the function (temporarily) avoids a wide refactor in one step.
  return;
}

export async function waitForHttpOk(
  url: string,
  options?: { timeoutMs?: number; intervalMs?: number },
): Promise<void> {
  const timeoutMs = options?.timeoutMs ?? 60_000;
  const intervalMs = options?.intervalMs ?? 1_000;

  const start = Date.now();
  while (true) {
    const ok = await httpStatusIs(url, 200);
    if (ok) return;

    if (Date.now() - start > timeoutMs) {
      throw new Error(
        `Timed out after ${timeoutMs}ms waiting for HTTP 200 from ${url}`,
      );
    }
    await sleep(intervalMs);
  }
}

export async function httpStatusIs(
  url: string,
  expected: number,
): Promise<boolean> {
  // Use global fetch (Node 18+). We only need the status code.
  try {
    const res = await fetch(url, { redirect: "manual" });
    return res.status === expected;
  } catch {
    return false;
  }
}

export type PublishCrateOptions = {
  cratePath: string; // relative to repo root
  registry: string;
  toolchain?: string; // e.g. "stable"
  allowDirty?: boolean; // default true
  removeLock?: boolean; // default true
  additionalArgs?: string[]; // e.g. ['--no-verify']

  /**
   * Token for the registry.
   *
   * Cargo **does not allow** setting `registries.<name>.token` via `--config` for security reasons.
   * Instead we pass the token via environment variable `CARGO_REGISTRIES_<NAME>_TOKEN`.
   *
   * Example:
   *   registry = "kellnr-test" => env var "CARGO_REGISTRIES_KELLNR_TEST_TOKEN"
   */
  registryToken?: string;
};

export async function publishCrate(
  options: PublishCrateOptions,
): Promise<void> {
  const repoRoot = repoRootFromTests();
  const absCratePath = path.resolve(repoRoot, options.cratePath);

  if (!fs.existsSync(absCratePath)) {
    throw new Error(`cratePath does not exist: ${absCratePath}`);
  }

  if (options.removeLock ?? true) {
    const lock = path.resolve(absCratePath, "Cargo.lock");
    try {
      fs.rmSync(lock, { force: true });
    } catch {
      // ignore
    }
  }

  const args: string[] = [];

  if (options.toolchain) args.push(`+${options.toolchain}`);

  // Fixed-port setup:
  // - Kellnr runs on fixed localhost:8000
  // - each test crate provides its own `.cargo/config.toml`
  // Therefore we do not override Cargo configuration here.
  args.push("publish", "--registry", options.registry);

  if (options.allowDirty ?? true) args.push("--allow-dirty");

  if (options.additionalArgs?.length) args.push(...options.additionalArgs);

  const env: Record<string, string> = {
    ...process.env,
  };

  // Cargo registry token env var format:
  //   CARGO_REGISTRIES_<REGISTRY_NAME_UPPER_SNAKE>_TOKEN
  // Where '-' becomes '_' and letters are uppercased.
  if (options.registryToken) {
    const envRegistryName = options.registry
      .toUpperCase()
      .replace(/[^A-Z0-9]+/g, "_")
      .replace(/^_+|_+$/g, "");
    env[`CARGO_REGISTRIES_${envRegistryName}_TOKEN`] = options.registryToken;
  }

  const res = await execOrThrow(
    "cargo",
    args,
    {
      cwd: absCratePath,
      env,
    },
    { description: `publish ${options.cratePath}` },
  );

  if (playDebug()) {
    log("debug", `cargo publish output:\n${res.stdout}\n${res.stderr}`);
  }
}

export function requireEnv(name: string): string {
  const v = process.env[name];
  expect(v, `Environment variable ${name} must be set`).toBeTruthy();
  return v as string;
}

/**
 * Legacy helper removed: withDockerContainer
 *
 * Use `withStartedContainer(...)` from `src/lib/docker.ts` together with
 * `startContainer(...)` to get:
 * - deterministic container naming
 * - automatic stop on teardown
 * - automatic Playwright log attachments on failure (or always)
 */

/**
 * A helper to make it explicit that tests using localhost:8000 cannot run in parallel.
 * Call this early in each smoke spec.
 */
export function restrictToSingleWorkerBecauseFixedPorts(): void {
  test.describe.configure({ mode: "serial" });
}

export async function assertDockerAvailable(): Promise<void> {
  const res = await exec("docker", ["version"]);
  if (res.exitCode !== 0) {
    throw new Error(
      `Docker does not seem available:\n${res.stdout}\n${res.stderr}`,
    );
  }
  if (playDebug()) log("debug", `docker version ok`);
}

/**
 * Assert that the Kellnr binary exists and is executable.
 * This should be called before starting tests that use local Kellnr.
 */
export function assertKellnrBinaryExists(): void {
  const binaryPath = getKellnrBinaryPath();
  if (!fs.existsSync(binaryPath)) {
    throw new Error(
      `Kellnr binary not found at ${binaryPath}. ` +
        `Run 'just npm-build && just build' first, or set KELLNR_BINARY_PATH.`,
    );
  }

  // Check if executable (on Unix systems)
  try {
    fs.accessSync(binaryPath, fs.constants.X_OK);
  } catch {
    throw new Error(`Kellnr binary at ${binaryPath} is not executable.`);
  }

  if (playDebug()) log("debug", `kellnr binary ok at ${binaryPath}`);
}

/**
 * Get the path to the Kellnr binary.
 * Can be overridden via KELLNR_BINARY_PATH environment variable.
 */
export function getKellnrBinaryPath(): string {
  if (process.env.KELLNR_BINARY_PATH) {
    return process.env.KELLNR_BINARY_PATH;
  }
  // Default: debug build at repo root
  const repoRoot = repoRootFromTests();
  return path.resolve(repoRoot, "target", "debug", "kellnr");
}

export async function fetchLatestReleasedKellnrImage(): Promise<string> {
  const image = "ghcr.io/kellnr/kellnr";
  const repo = "kellnr/kellnr";

  type Version = { major: number; minor: number; patch: number; pre?: string };

  const parseVersion = (raw: string): Version | null => {
    // tags are semver: x.y.z, optionally prefixed with v,
    // optionally with prerelease: x.y.z-rc.1 / -beta.2 / -rc1
    const s = raw.startsWith("v") ? raw.slice(1) : raw;
    const m = s.match(
      /^([0-9]+)\.([0-9]+)\.([0-9]+)(?:-([0-9A-Za-z.-]+))?$/,
    );
    if (!m) return null;
    return {
      major: Number(m[1]),
      minor: Number(m[2]),
      patch: Number(m[3]),
      pre: m[4],
    };
  };

  const cmpVersion = (a: Version, b: Version): number => {
    if (a.major !== b.major) return a.major - b.major;
    if (a.minor !== b.minor) return a.minor - b.minor;
    if (a.patch !== b.patch) return a.patch - b.patch;

    // SemVer precedence: stable > prerelease
    const aPre = a.pre;
    const bPre = b.pre;
    if (!aPre && bPre) return 1;
    if (aPre && !bPre) return -1;
    if (!aPre && !bPre) return 0;

    // SemVer prerelease precedence:
    // - compare dot-separated identifiers
    // - numeric identifiers are compared numerically
    // - numeric identifiers have lower precedence than non-numeric
    // - if equal so far, shorter identifier list has higher precedence
    const splitIds = (pre: string): Array<string | number> =>
      pre.split(".").map((id) => (/^[0-9]+$/.test(id) ? Number(id) : id));

    const aIds = splitIds(aPre!);
    const bIds = splitIds(bPre!);

    const len = Math.min(aIds.length, bIds.length);
    for (let i = 0; i < len; i++) {
      const ai = aIds[i];
      const bi = bIds[i];
      if (ai === bi) continue;

      const aIsNum = typeof ai === "number";
      const bIsNum = typeof bi === "number";

      if (aIsNum && !bIsNum) return -1;
      if (!aIsNum && bIsNum) return 1;

      if (aIsNum && bIsNum) return (ai as number) - (bi as number);
      return String(ai).localeCompare(String(bi));
    }

    if (aIds.length !== bIds.length) return aIds.length < bIds.length ? 1 : -1;
    return 0;
  };

  const fetchGhcrToken = async (): Promise<string | null> => {
    // Anonymous pull token is usually sufficient and avoids GitHub API rate limits.
    const tokenUrl = `https://ghcr.io/token?service=ghcr.io&scope=repository:${repo}:pull`;
    const res = await fetch(tokenUrl, {
      headers: { "user-agent": "kellnr-tests/1.0" },
    });
    if (!res.ok) return null;

    const data = (await res.json().catch(() => null)) as any;
    return typeof data?.token === "string" ? data.token : null;
  };

  const token = await fetchGhcrToken();

  const tagsUrl = `https://ghcr.io/v2/${repo}/tags/list?n=1000`;
  const headers: Record<string, string> = {
    "user-agent": "kellnr-tests/1.0",
    accept: "application/json",
  };
  if (token) headers.authorization = `Bearer ${token}`;

  const res = await fetch(tagsUrl, { headers });
  const bodyText = await res.text();

  if (!res.ok) {
    throw new Error(
      `GHCR tags request failed while fetching ${image} tags. ` +
      `Status: ${res.status}. Body prefix: ${bodyText.slice(0, 200)}`,
    );

  }

  let payload: unknown;
  try {
    payload = JSON.parse(bodyText);
  } catch {
    throw new Error(
      `Failed to parse JSON response from GHCR tags endpoint. Response prefix: ${bodyText.slice(0, 200)}`,
    );
  }

  const tags = (payload as any)?.tags;
  if (!Array.isArray(tags)) {
    throw new Error(
      `Unexpected GHCR tags response shape. Response prefix: ${bodyText.slice(0, 200)}`,
    );
  }

  // Filter to semver-like tags, ignore "latest"
  const versionTags = tags
    .filter((t: unknown): t is string => typeof t === "string")
    .filter((t) => t !== "latest")
    .map((t) => ({ tag: t, v: parseVersion(t) }))
    .filter((x) => x.v !== null) as Array<{ tag: string; v: Version }>;

  if (versionTags.length === 0) {
    throw new Error(
      `No semver-like tags found on ${image} (excluding "latest").`,
    );

  }

  versionTags.sort((a, b) => cmpVersion(a.v, b.v));
  const best = versionTags[versionTags.length - 1].tag;
  const bestNoV = best.startsWith("v") ? best.slice(1) : best;

  return `${image}:${bestNoV}`;
}

