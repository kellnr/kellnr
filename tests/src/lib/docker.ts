import fs from "node:fs";
import path from "node:path";
import type { TestInfo } from "@playwright/test";
import type { BeforeAllTestInfo } from "../testUtils";
import { execa } from "execa";
import {
  GenericContainer,
  Network,
  Wait,
  type StartedTestContainer,
  type StartedNetwork,
} from "testcontainers";

/**
 * Central docker/testcontainers helper library for tests.
 *
 * Goals:
 * - Keep test specs idiomatic: use `testcontainers` directly (option 2)
 * - Avoid duplicated boilerplate: container naming, network creation, log capture, bind mounts
 * - Integrate nicely with Playwright: attach logs on failure / always
 *
 * Notes:
 * - globalSetup is responsible for building the shared Kellnr integration image.
 * - This module *can* build test-specific helper images (e.g. MinIO for S3 test) via
 *   `ImageFromDockerfile`, so individual specs don't shell out to the Docker CLI.
 *
 * Logging:
 * - Docker Desktop can hang if we try to read container logs until end-of-stream after a long run.
 * - Instead we start a background log streamer right after container start and keep appending
 *   to a file (and optionally to stdout). At teardown we attach the file without waiting for
 *   the Docker log stream to end.
 */

export type PortBindings = Record<number, number>; // containerPort -> hostPort (fixed mapping)

export type StartContainerOptions = {
  /**
   * Docker image name, e.g. "ghcr.io/kellnr/kellnr:latest" or "minio/minio:RELEASE..."
   */
  image: string;

  /**
   * Human-centric name prefix. The final container name is suffixed to ensure uniqueness.
   */
  name: string;

  /**
   * Environment variables.
   */
  env?: Record<string, string>;

  /**
   * Bind mounts (hostPath -> containerPath).
   *
   * Example:
   *   bindMounts: { "/host/tmp/kdata": "/opt/kdata" }
   */
  bindMounts?: Record<string, string>;

  /**
   * Optional network to attach the container to (for inter-container communication).
   */
  network?: StartedNetwork;

  /**
   * Optional network aliases (e.g. ["minio"]).
   */
  networkAliases?: string[];

  /**
   * Container port -> host port.
   *
   * When provided, the given host port is bound to the container port via testcontainers'
   * PortWithBinding support (e.g. `{ container: 8000, host: 8000 }`).
   *
   * This restores the fixed-port setup where Kellnr always runs on localhost:8000 and
   * crate-local `.cargo/config.toml` can remain static.
   */
  ports?: PortBindings;

  /**
   * Expose container ports to random host ports.
   *
   * Use this when you need to access a container port from the host but don't need
   * a specific host port. Call `container.getMappedPort(containerPort)` to get the
   * assigned host port.
   */
  exposedPorts?: number[];

  /**
   * Command override passed to Docker (ENTRYPOINT stays, CMD replaced).
   * Use for images like minio where you might want e.g. ["server","/data"].
   */
  cmd?: string[];

  /**
   * Additional wait strategy. For HTTP readiness, prefer `waitForHttp(...)`.
   */
  waitFor?:
    | ReturnType<typeof Wait.forLogMessage>
    | ReturnType<typeof Wait.forListeningPorts>
    | ReturnType<typeof Wait.forHttp>;

  /**
   * Optional labels (handy for debugging / cleanup).
   */
  labels?: Record<string, string>;
};

export type Started = {
  container: StartedTestContainer;
  name: string;

  /**
   * If log streaming is enabled, this is the file path where logs are being appended.
   */
  logsFilePath?: string;

  /**
   * Stops background log streaming (if enabled).
   */
  stopLogStreaming?: () => void;
};

function nowIsoNoMs(): string {
  return new Date()
    .toISOString()
    .replace(/\.\d{3}Z$/, "Z")
    .replace(/[:]/g, "-");
}

function sluggifyForName(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40);
}

function ensureDir(dir: string): void {
  fs.mkdirSync(dir, { recursive: true });
}

function testsRoot(): string {
  // Playwright runs with cwd tests/ (see existing tests code). Keep robust anyway.
  return path.resolve(process.cwd());
}

function projectTmpDir(testInfo: TestInfo | BeforeAllTestInfo): string {
  const dir = path.resolve(
    testsRoot(),
    "tmp",
    `${testInfo.title.replace(/[^\w.-]+/g, "_")}-${testInfo.workerIndex}`,
  );
  ensureDir(dir);
  return dir;
}

/**
 * Produce a stable-ish, unique Docker container name.
 * Docker names must be unique on a daemon; even across workers.
 */
export function uniqueContainerName(base: string, testInfo?: TestInfo | BeforeAllTestInfo): string {
  const slug = sluggifyForName(base);
  const worker = testInfo ? `w${testInfo.workerIndex}` : "w?";
  return `${slug}-${worker}-${Date.now()}`;
}

/**
 * Start a network for a test.
 *
 * Use this when a container needs to reach another container via alias (e.g. Kellnr -> minio).
 */
export async function createNetwork(
  name: string,
  testInfo?: TestInfo | BeforeAllTestInfo,
): Promise<StartedNetwork> {
  const netName = uniqueContainerName(name, testInfo);

  // `Network` in the currently used `testcontainers` API does not support `.withName(...)`.
  // It will create a uniquely named network on start; we keep `netName` for logging/debugging.
  const network: Network = await new Network().start();
  return network;
}

/**
 * Build the custom MinIO image used by the S3 smoke test from the repository Dockerfile.
 *
 * The `testcontainers` build-from-Dockerfile helper is not available in the currently used
 * `testcontainers` package API here, so we centralize a Docker CLI fallback in this library.
 *
 * This keeps the spec code idiomatic (it just calls one helper) while remaining compatible.
 *
 * Dockerfile/context mirror the previous behavior:
 * - Dockerfile: <repoRoot>/tests/test-s3-storage/Dockerfile
 * - Context:    <repoRoot>/tests/test-s3-storage
 *
 * NOTE: This function assumes it's called from within tests (process.cwd() == tests root).
 */
export async function buildS3MinioImage(options: {
  imageName: string;
  cratesBucket: string;
  cratesioBucket: string;
}): Promise<void> {
  const repoRoot = path.resolve(process.cwd(), "..");

  // We build the MinIO fixture image from `tests/fixtures/test-s3-storage`.
  const contextDir = path.resolve(
    repoRoot,
    "tests",
    "fixtures",
    "test-s3-storage",
  );
  const dockerfile = path.resolve(contextDir, "Dockerfile");

  const args = [
    "build",
    "-t",
    options.imageName,
    "-f",
    dockerfile,
    "--build-arg",
    `CRATES_BUCKET=${options.cratesBucket}`,
    "--build-arg",
    `CRATESIO_BUCKET=${options.cratesioBucket}`,
    contextDir,
  ];

  const res = await execa("docker", args, {
    cwd: repoRoot,
    stdio: "inherit",
  });

  if (res.exitCode !== 0) {
    throw new Error(
      `Failed to build MinIO image ${options.imageName} (exitCode=${res.exitCode})`,
    );
  }
}

/**
 * Basic log attachment helper.
 *
 * We attach via a file path so Playwright can keep it as an artifact reliably.
 * `testcontainers` logs can be streamed but for determinism we fetch at teardown time.
 */
export async function attachContainerLogs(
  testInfo: TestInfo,
  _container: StartedTestContainer,
  opts?: { name?: string; filePath?: string },
): Promise<void> {
  const base = opts?.name ?? "container";
  const safeBase = base.replace(/[^\w.-]+/g, "_");

  const file =
    opts?.filePath ??
    path.resolve(
      projectTmpDir(testInfo),
      `${safeBase}.${nowIsoNoMs()}.docker.log`,
    );

  try {
    await testInfo.attach(`${safeBase}-docker-logs`, {
      path: file,
      contentType: "text/plain",
    });
  } catch (e) {
    // Attaching logs should never fail the test; it's best-effort.
    const msg = (e as Error)?.message ?? String(e);
    await testInfo.attach(`${safeBase}-docker-logs-error`, {
      body: Buffer.from(
        `Failed to attach logs file (${file}): ${msg}\n`,
        "utf8",
      ),
      contentType: "text/plain",
    });
  }
}

async function startStreamingContainerLogs(options: {
  container: StartedTestContainer;
  filePath: string;
  stdoutPrefix?: string;
  alsoStdout?: boolean;
}): Promise<{
  stop: () => void;
}> {
  // Best-effort: never throw from here; logging must not break tests.
  const { container, filePath } = options;

  const fd = fs.openSync(filePath, "a");
  let stopped = false;

  const writeLine = (line: string) => {
    if (stopped) return;
    fs.writeSync(fd, line);
  };

  try {
    const stream = await container.logs();

    const onData = (chunk: unknown) => {
      const s =
        typeof chunk === "string"
          ? chunk
          : Buffer.isBuffer(chunk)
            ? chunk.toString("utf8")
            : String(chunk);

      // Ensure log file is line-oriented and readable even if chunks don't end with newline.
      const line = s.endsWith("\n") ? s : `${s}\n`;

      writeLine(line);

      if (options.alsoStdout) {
        const prefix = options.stdoutPrefix ? `[${options.stdoutPrefix}] ` : "";
        // eslint-disable-next-line no-console
        console.log(prefix + line.trimEnd());
      }
    };

    const onError = (err: unknown) => {
      const msg = err instanceof Error ? err.message : String(err);
      writeLine(`[stream-error] ${msg}\n`);
    };

    stream.on("data", onData);
    stream.on("error", onError);

    // Do NOT await end/close; Docker Desktop can keep the stream open indefinitely.
    return {
      stop: () => {
        if (stopped) return;
        stopped = true;

        try {
          stream.off("data", onData);
          stream.off("error", onError);
        } catch {
          // ignore
        }

        try {
          fs.closeSync(fd);
        } catch {
          // ignore
        }
      },
    };
  } catch (e) {
    const msg = (e as Error)?.message ?? String(e);
    writeLine(`[stream-init-error] ${msg}\n`);
    return {
      stop: () => {
        if (stopped) return;
        stopped = true;
        try {
          fs.closeSync(fd);
        } catch {
          // ignore
        }
      },
    };
  }
}

/**
 * Start a container with common concerns handled in one place:
 * - deterministic name
 * - optional network/aliases
 * - optional bind mounts
 * - optional host port bindings
 * - optional wait strategy
 */
export async function startContainer(
  options: StartContainerOptions,
  testInfo: TestInfo | BeforeAllTestInfo,
): Promise<Started> {
  const name = uniqueContainerName(options.name, testInfo);

  let container = new GenericContainer(options.image).withName(name);

  if (options.env) {
    for (const [k, v] of Object.entries(options.env)) {
      container = container.withEnvironment({ [k]: v });
    }
  }

  if (options.labels) {
    container = container.withLabels(options.labels);
  }

  if (options.cmd?.length) {
    container = container.withCommand(options.cmd);
  }

  if (options.bindMounts) {
    for (const [hostPath, containerPath] of Object.entries(
      options.bindMounts,
    )) {
      container = container.withBindMounts([
        { source: hostPath, target: containerPath },
      ]);
    }
  }

  if (options.ports) {
    // Fixed host port mapping support:
    // testcontainers accepts PortWithBinding objects in `withExposedPorts`, e.g.
    //   { container: 8000, host: 8000 }
    //
    // This allows a stable localhost:8000 setup (legacy fixed-port behavior).
    for (const [containerPortStr, hostPort] of Object.entries(options.ports)) {
      const containerPort = Number(containerPortStr);
      container = container.withExposedPorts({
        container: containerPort,
        host: hostPort,
      });
    }
  }

  if (options.exposedPorts?.length) {
    // Expose ports to random host ports (testcontainers picks an available port).
    // Use container.getMappedPort(containerPort) to get the assigned host port.
    container = container.withExposedPorts(...options.exposedPorts);
  }

  if (options.network) {
    container = container.withNetwork(options.network);
    if (options.networkAliases?.length) {
      container = container.withNetworkAliases(...options.networkAliases);
    }
  }

  if (options.waitFor) {
    container = container.withWaitStrategy(options.waitFor);
  } else {
    // Default: wait for listening ports, which fits most services.
    container = container.withWaitStrategy(Wait.forListeningPorts());
  }

  const started = await container.start();

  // Start log streaming immediately so we never have to wait for an "end" event later.
  // This avoids teardown hangs on Docker Desktop while still producing useful artifacts.
  const logsFilePath = path.resolve(
    projectTmpDir(testInfo),
    `${name}.${nowIsoNoMs()}.docker.log`,
  );

  // Ensure file exists even if stream init fails later.
  try {
    fs.writeFileSync(logsFilePath, "", { flag: "a" });
  } catch {
    // ignore
  }

  const streamer = await startStreamingContainerLogs({
    container: started,
    filePath: logsFilePath,
    stdoutPrefix: name,
    alsoStdout: false,
  });

  return {
    container: started,
    name,
    logsFilePath,
    stopLogStreaming: streamer.stop,
  };
}

/**
 * Convenience: create a wait strategy for an HTTP endpoint within the container.
 *
 * Example:
 *   waitFor: waitForHttp(8000, "/health")
 */
export function waitForHttp(
  port: number,
  pathName: string,
  opts?: { statusCode?: number; startupTimeoutMs?: number },
) {
  const statusCode = opts?.statusCode ?? 200;
  const startupTimeoutMs = opts?.startupTimeoutMs ?? 60_000;

  return Wait.forHttp(pathName)
    .forPort(port)
    .forStatusCode(statusCode)
    .withStartupTimeout(startupTimeoutMs);
}

/**
 * Start the MinIO container for the S3 smoke test on the given network with a stable alias.
 *
 * This is a thin convenience wrapper around `startContainer` to avoid repeating:
 * - env wiring
 * - network alias
 * - (optional) wait strategy
 */
export async function startS3MinioContainer(
  options: {
    name: string;
    image: string;
    network: StartedNetwork;
    rootUser: string;
    rootPassword: string;
    /**
     * If true, expose port 9000 to a random host port.
     * Use container.getMappedPort(9000) to get the assigned port.
     * Required when Kellnr runs locally (not in Docker) and needs to access MinIO.
     */
    exposeToHost?: boolean;
  },
  testInfo?: TestInfo | BeforeAllTestInfo,
): Promise<Started> {
  return await startContainer(
    {
      name: options.name,
      image: options.image,
      network: options.network,
      networkAliases: ["minio"],
      env: {
        MINIO_ROOT_USER: options.rootUser,
        MINIO_ROOT_PASSWORD: options.rootPassword,
      },
      // Expose port 9000 to a random host port if requested (for local Kellnr access)
      ...(options.exposeToHost ? { exposedPorts: [9000] } : {}),
      // MinIO images typically listen quickly; the default listening-ports wait is sufficient.
      // If you want stricter readiness, replace with `waitFor: waitForHttp(9000, "/minio/health/live")`
    },
    testInfo,
  );
}

/**
 * Run a function with a started container and guaranteed cleanup.
 *
 * - On error: attaches logs then rethrows
 * - Always: stops container
 * - Optional: always collects logs (even on success)
 */
export async function withStartedContainer<T>(
  testInfo: TestInfo,
  started: Started,
  fn: (container: StartedTestContainer) => Promise<T>,
  opts?: { alwaysCollectLogs?: boolean },
): Promise<T> {
  try {
    return await fn(started.container);
  } catch (e) {
    await attachContainerLogs(testInfo, started.container, {
      name: started.name,
      filePath: started.logsFilePath,
    });
    throw e;
  } finally {
    try {
      if (opts?.alwaysCollectLogs) {
        await attachContainerLogs(testInfo, started.container, {
          name: started.name,
          filePath: started.logsFilePath,
        });
      }
    } finally {
      // Stop log streaming before stopping the container to avoid dangling listeners/handles.
      try {
        started.stopLogStreaming?.();
      } catch {
        // ignore
      }

      // Best-effort cleanup
      await started.container.stop().catch(() => {});
    }
  }
}

/**
 * Run a function with a started network and guaranteed cleanup.
 */
export async function withStartedNetwork<T>(
  startedNetwork: StartedNetwork,
  fn: (network: StartedNetwork) => Promise<T>,
): Promise<T> {
  try {
    return await fn(startedNetwork);
  } finally {
    await startedNetwork.stop().catch(() => {});
  }
}
