import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import net from "node:net";
import { setTimeout as sleep } from "node:timers/promises";
import { execa, ExecaError, type Options as ExecaOptions } from "execa";
import { expect, test, type TestInfo } from "@playwright/test";

type LogLevel = "info" | "debug" | "error";

function nowIsoNoMs(): string {
  // Keep filenames stable-ish and readable.
  return new Date()
    .toISOString()
    .replace(/\.\d{3}Z$/, "Z")
    .replace(/[:]/g, "-");
}

function isCi(): boolean {
  return !!process.env.CI;
}

function playDebug(): boolean {
  return (
    process.env.DEBUG?.includes("tests2") || process.env.TESTS2_DEBUG === "1"
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

export function repoRootFromTests2(): string {
  // tests2/ is at repoRoot/tests2
  return path.resolve(process.cwd(), "..");
}

export function tests2Root(): string {
  return path.resolve(process.cwd());
}

export function logsDir(): string {
  return path.resolve(tests2Root(), "logs");
}

export function projectTmpDir(testInfo: TestInfo): string {
  // Per-test directory for ephemeral artifacts (docker logs, etc.).
  const dir = path.resolve(
    tests2Root(),
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

export type DockerRunOptions = {
  name: string;
  image: string;
  ports?: Record<number | string, number | string>; // host->container
  env?: Record<string, string>;
  extraArgs?: string[]; // additional docker args before image (e.g. ['--network','s3-net','-v',...])
  detach?: boolean; // default true
};

export async function dockerBuild(options: {
  tag: string;
  dockerfile?: string; // default: tests/Dockerfile or caller-specified
  contextDir: string;
  buildArgs?: Record<string, string>;
}): Promise<void> {
  const args: string[] = ["build", "-t", options.tag];

  if (options.dockerfile) args.push("-f", options.dockerfile);

  if (options.buildArgs) {
    for (const [k, v] of Object.entries(options.buildArgs)) {
      args.push("--build-arg", `${k}=${v}`);
    }
  }

  args.push(options.contextDir);

  await execOrThrow(
    "docker",
    args,
    {},
    { description: `docker build ${options.tag}` },
  );
}

/**
 * Allocate a free localhost TCP port at the time of allocation.
 *
 * This is used to run docker containers with dynamic host port mappings so
 * multiple smoke tests can run in parallel without "port already allocated".
 *
 * Note: like any ephemeral-port allocation, there is a small TOCTOU risk
 * between releasing the port and Docker binding it, but in practice it works
 * well for CI and local parallel runs.
 */
export async function allocateFreeLocalhostPort(): Promise<number> {
  return await new Promise<number>((resolve, reject) => {
    const server = net.createServer();

    server.on("error", (err) => reject(err));

    // Bind to port 0 to let the OS pick an ephemeral free port.
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        server.close(() => reject(new Error("Failed to allocate a TCP port")));
        return;
      }

      const port = address.port;
      server.close((closeErr) => {
        if (closeErr) {
          reject(closeErr);
          return;
        }
        resolve(port);
      });
    });
  });
}

export async function dockerImageExists(image: string): Promise<boolean> {
  // `docker image inspect` returns non-zero if the image doesn't exist locally
  const res = await exec("docker", ["image", "inspect", image]);
  return res.exitCode === 0;
}

/**
 * Ensures the local Kellnr test image exists. This mirrors the Lua test runner behavior
 * which builds `kellnr-test:local` before running individual tests.
 *
 * By default, it uses:
 * - Dockerfile: tests/Dockerfile
 * - Build context: repo root (../ from tests2)
 * - Build arg: KELLNR_VERSION=local
 */
export async function ensureLocalKellnrTestImage(image: string): Promise<void> {
  // globalSetup is responsible for building the shared image once for the whole test run.
  // Keeping this helper as an existence check makes tests resilient when run standalone.
  const exists = await dockerImageExists(image);
  if (exists) return;

  throw new Error(
    `Docker image not found locally: ${image}. ` +
      `Run the test suite via Playwright so globalSetup can build it, ` +
      `or build it manually (e.g. docker build -f tests/Dockerfile -t ${image} ..).`,
  );
}

export async function dockerPull(image: string): Promise<void> {
  await execOrThrow(
    "docker",
    ["pull", image],
    {},
    { description: `docker pull ${image}` },
  );
}

export async function dockerNetworkCreate(name: string): Promise<void> {
  // Creating twice fails; keep it simple and idempotent.
  const res = await exec("docker", ["network", "create", name]);
  if (res.exitCode !== 0) {
    // If it already exists, fine.
    const out = `${res.stdout}\n${res.stderr}`;
    if (!out.toLowerCase().includes("already exists")) {
      throw new Error(`Failed to create docker network ${name}\n${out}`);
    }
  }
}

export async function dockerNetworkRemove(name: string): Promise<void> {
  // Removing non-existent networks fails; ignore that case.
  const res = await exec("docker", ["network", "rm", name]);
  if (res.exitCode !== 0) {
    const out = `${res.stdout}\n${res.stderr}`;
    if (!out.toLowerCase().includes("no such network")) {
      throw new Error(`Failed to remove docker network ${name}\n${out}`);
    }
  }
}

export async function dockerRun(opts: DockerRunOptions): Promise<void> {
  const args: string[] = ["run", "--rm", "--name", opts.name];

  if (opts.ports) {
    for (const [host, container] of Object.entries(opts.ports)) {
      args.push("-p", `${host}:${container}`);
    }
  }

  if (opts.env) {
    for (const [k, v] of Object.entries(opts.env)) {
      args.push("-e", `${k}=${v}`);
    }
  }

  if (opts.extraArgs?.length) args.push(...opts.extraArgs);

  if (opts.detach ?? true) args.push("-d");

  args.push(opts.image);

  await execOrThrow(
    "docker",
    args,
    {},
    { description: `docker run ${opts.name}` },
  );
}

export async function dockerStop(name: string): Promise<void> {
  // Ignore stop failures when container doesn't exist (helps cleanup).
  const res = await exec("docker", ["stop", name]);
  if (res.exitCode !== 0) {
    const out = `${res.stdout}\n${res.stderr}`.toLowerCase();
    if (!out.includes("no such container")) {
      throw new Error(
        `Failed to stop container ${name}\n${res.stdout}\n${res.stderr}`,
      );
    }
  }
}

export async function dockerLogs(name: string): Promise<string> {
  const res = await exec("docker", ["logs", name]);
  // docker logs exits non-zero if the container doesn't exist; return best-effort output
  return [res.stdout, res.stderr].filter(Boolean).join("\n");
}

export async function writeDockerLogsArtifact(
  testInfo: TestInfo,
  containerName: string,
  filenameBase?: string,
) {
  try {
    const logs = await dockerLogs(containerName);
    const base = filenameBase ?? containerName;
    const file = path.resolve(
      projectTmpDir(testInfo),
      `${base}.${nowIsoNoMs()}.docker.log`,
    );
    fs.writeFileSync(file, logs, "utf8");

    await testInfo.attach(`${base}-docker-logs`, {
      path: file,
      contentType: "text/plain",
    });
  } catch (e) {
    log(
      "error",
      `Failed to collect docker logs for ${containerName}: ${(e as Error).message}`,
    );
  }
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
   * Optional: override registry endpoints for this cargo publish invocation.
   * This is required for parallel smoke tests that run Kellnr on dynamic host ports.
   *
   * Example:
   *   registryBaseUrl: "http://localhost:12345"
   *
   * NOTE:
   * We do **not** rely on CARGO_HOME for this, because the crate-local `.cargo/config.toml`
   * can still win in practice. Instead we use `cargo --config ...` overrides which have
   * the highest precedence.
   */
  registryBaseUrl?: string;

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
  const repoRoot = repoRootFromTests2();
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

  // Force Cargo to ignore crate-local `.cargo/config.toml` by overriding configuration
  // via CLI `--config`, which has the highest precedence.
  //
  // This is necessary for parallel tests where each Kellnr instance runs on a different
  // dynamic port, but the crates' `.cargo/config.toml` hardcodes `localhost:8000`.
  if (options.registryBaseUrl) {
    const baseUrl = options.registryBaseUrl.replace(/\/+$/, "");
    const registryName = options.registry;

    args.push(
      "--config",
      `registries.${registryName}.index="sparse+${baseUrl}/api/v1/crates/"`,
    );

    // Cargo blocks `registries.<name>.token` via `--config` for security reasons.
    // We set it via environment variable instead (see below when running cargo).

    // Mirror the crate config: replace crates-io with kellnr cratesio sparse endpoint.
    // This is important because publishing runs can still touch crates-io for dependency resolution.
    args.push(
      "--config",
      `source.crates-io.replace-with="${registryName}-sparse-cratesio"`,
    );
    args.push(
      "--config",
      `source.${registryName}-sparse-cratesio.registry="sparse+${baseUrl}/api/v1/cratesio/"`,
    );

    // Preserve token-based credential provider behavior as in the crate configs.
    args.push(
      "--config",
      `registry.global-credential-providers=["cargo:token"]`,
    );
  }

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

export async function withDockerContainer(
  testInfo: TestInfo,
  containerName: string,
  fn: () => Promise<void>,
  opts?: { alwaysCollectLogs?: boolean },
): Promise<void> {
  try {
    await fn();
  } catch (e) {
    await writeDockerLogsArtifact(testInfo, containerName);
    throw e;
  } finally {
    // Best-effort cleanup
    await dockerStop(containerName);
    if (opts?.alwaysCollectLogs) {
      await writeDockerLogsArtifact(testInfo, containerName);
    }
  }
}

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
