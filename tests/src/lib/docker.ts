import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import type { TestInfo } from "@playwright/test";
import type { BeforeAllTestInfo } from "../testUtils";
import { GenericContainer, Wait, type StartedTestContainer } from "testcontainers";

/**
 * Central docker/testcontainers helper library for tests.
 *
 * Goals:
 * - Keep test specs idiomatic: use `testcontainers` directly (option 2)
 * - Avoid duplicated boilerplate: container naming, log capture, bind mounts
 * - Integrate nicely with Playwright: attach logs on failure / always
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
   * Docker image name, e.g. "ghcr.io/kellnr/kellnr:latest"
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
   * Command override passed to Docker (ENTRYPOINT stays, CMD replaced).
   */
  cmd?: string[];

  /**
   * Additional wait strategy.
   */
  waitFor?:
    | ReturnType<typeof Wait.forLogMessage>
    | ReturnType<typeof Wait.forListeningPorts>
    | ReturnType<typeof Wait.forHttp>;

  /**
   * Extra /etc/hosts entries (e.g. `{ host: "host.docker.internal", ipAddress: "host-gateway" }`).
   * On Linux, use this to map host.docker.internal to the Docker host.
   */
  extraHosts?: Array<{ host: string; ipAddress: string }>;
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

  if (options.extraHosts?.length) {
    container = container.withExtraHosts(options.extraHosts);
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
 * Reserve a free TCP port on the host by briefly binding to it, then releasing it.
 * Another process could take the port before the caller binds it.
 */
export async function reserveFreePort(): Promise<number> {
  return await new Promise((resolve, reject) => {
    const server = net.createServer();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => {
        if (address && typeof address === "object") {
          resolve(address.port);
        } else {
          reject(new Error("Failed to reserve a free port"));
        }
      });
    });
  });
}
