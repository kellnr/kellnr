/**
 * Local Kellnr process management for UI tests.
 *
 * This module provides helpers to run Kellnr as a local process instead of in Docker,
 * significantly speeding up test runs by eliminating Docker image build overhead.
 *
 * Key features:
 * - Data isolation via unique `/tmp/kellnr-test-ui/<uuid>/` directories
 * - Automatic cleanup of data directories
 * - Process lifecycle management with proper signal handling
 */

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { randomUUID } from "node:crypto";
import type { ChildProcess } from "node:child_process";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import type { TestInfo } from "@playwright/test";
import { type BeforeAllTestInfo, getKellnrBinaryPath } from "../testUtils";

/**
 * Base directory for test data directories.
 */
const TEST_DATA_BASE_DIR = "/tmp/kellnr-test-ui";

/**
 * Result of starting a local Kellnr instance.
 */
export type StartedLocalKellnr = {
  /** The spawned child process */
  process: ChildProcess;

  /** Base URL for accessing the Kellnr server (e.g., "http://localhost:8000") */
  baseUrl: string;

  /** Path to the data directory for this instance */
  dataDir: string;

  /** Path to the log file for this instance */
  logFile: string;

  /** Stop the process and clean up the data directory */
  stop: () => Promise<void>;
};

export type StartLocalKellnrOptions = {
  /**
   * Name prefix for logging/debugging purposes.
   */
  name: string;

  /**
   * Extra environment variables to pass to the Kellnr process.
   * These are merged with the default environment variables.
   */
  env?: Record<string, string>;

  /**
   * Port to run Kellnr on. Defaults to 8000.
   */
  port?: number;

  /**
   * Log level. Defaults to "debug".
   */
  logLevel?: string;

  /**
   * Web server log level. Defaults to "debug".
   */
  webLogLevel?: string;

  /**
   * Whether to keep the data directory after stopping.
   * Useful for debugging. Defaults to false.
   */
  keepDataDir?: boolean;

  /**
   * Whether to also write logs to stdout.
   * Useful for debugging. Defaults to false.
   */
  logToStdout?: boolean;

  /**
   * Path to use for the health check. Defaults to "/".
   * Set this when using KELLNR_ORIGIN__PATH to match the path prefix.
   */
  healthCheckPath?: string;
};

/**
 * Ensure the test data base directory exists.
 */
function ensureTestDataBaseDir(): void {
  fs.mkdirSync(TEST_DATA_BASE_DIR, { recursive: true });
}

/**
 * Create a unique data directory for a test instance.
 */
function createDataDir(name: string): string {
  ensureTestDataBaseDir();
  const uuid = randomUUID();
  const safeName = name.replace(/[^\w.-]+/g, "_").slice(0, 40);
  const dirName = `${safeName}-${uuid}`;
  const dataDir = path.resolve(TEST_DATA_BASE_DIR, dirName);
  fs.mkdirSync(dataDir, { recursive: true });
  return dataDir;
}

/**
 * Remove a data directory.
 */
function removeDataDir(dataDir: string): void {
  try {
    fs.rmSync(dataDir, { recursive: true, force: true });
  } catch {
    // Best effort cleanup
  }
}

/**
 * Wait for HTTP 200 on a URL.
 */
async function waitForHttpOk(
  url: string,
  options?: { timeoutMs?: number; intervalMs?: number },
): Promise<void> {
  const timeoutMs = options?.timeoutMs ?? 60_000;
  const intervalMs = options?.intervalMs ?? 500;
  const start = Date.now();

  while (true) {
    try {
      const res = await fetch(url, { redirect: "manual" });
      if (res.status === 200) return;
    } catch {
      // Server not ready yet
    }

    if (Date.now() - start > timeoutMs) {
      throw new Error(
        `Timed out after ${timeoutMs}ms waiting for HTTP 200 from ${url}`,
      );
    }
    await sleep(intervalMs);
  }
}

/**
 * Start a local Kellnr process for testing.
 *
 * This function:
 * 1. Creates a unique data directory for the instance
 * 2. Spawns the Kellnr binary with appropriate environment variables
 * 3. Waits for the server to become ready
 * 4. Returns a handle with a stop() method for cleanup
 *
 * @example
 * ```typescript
 * const kellnr = await startLocalKellnr({ name: "test-login" });
 * // ... run tests against kellnr.baseUrl ...
 * await kellnr.stop();
 * ```
 */
export async function startLocalKellnr(
  options: StartLocalKellnrOptions,
  _testInfo?: TestInfo | BeforeAllTestInfo,
): Promise<StartedLocalKellnr> {
  const binaryPath = getKellnrBinaryPath();
  const port = options.port ?? 8000;
  const logLevel = options.logLevel ?? "debug";
  const webLogLevel = options.webLogLevel ?? "debug";
  const baseUrl = `http://localhost:${port}`;

  // Check if user provided an explicit data directory via env
  const userProvidedDataDir = options.env?.KELLNR_REGISTRY__DATA_DIR;
  const isExternalDataDir = !!userProvidedDataDir;

  // Create or use existing data directory
  const dataDir = isExternalDataDir
    ? userProvidedDataDir
    : createDataDir(options.name);

  // Ensure data directory exists (in case it's an external directory that needs to be created)
  fs.mkdirSync(dataDir, { recursive: true });

  // Create log file
  const logFile = path.resolve(dataDir, "kellnr.log");

  // Build environment
  const env: Record<string, string> = {
    ...process.env,
    // Core settings
    KELLNR_REGISTRY__DATA_DIR: dataDir,
    KELLNR_ORIGIN__PORT: String(port),
    KELLNR_ORIGIN__HOSTNAME: "localhost",
    // Logging
    KELLNR_LOG__LEVEL: logLevel,
    KELLNR_LOG__LEVEL_WEB_SERVER: webLogLevel,
    // Merge user-provided environment
    ...(options.env ?? {}),
  };

  // Open log file for writing
  const logFd = fs.openSync(logFile, "w");
  const logStream = fs.createWriteStream("", { fd: logFd });

  // Spawn the process
  const childProcess = spawn(binaryPath, [], {
    cwd: dataDir,
    env,
    stdio: ["ignore", "pipe", "pipe"],
    detached: false,
  });

  // Pipe stdout/stderr to log file (and optionally to console)
  const handleOutput = (stream: NodeJS.ReadableStream, prefix: string) => {
    stream.on("data", (chunk: Buffer) => {
      const text = chunk.toString("utf8");
      logStream.write(`[${prefix}] ${text}`);
      if (options.logToStdout) {
        process.stdout.write(`[${options.name}:${prefix}] ${text}`);
      }
    });
  };

  if (childProcess.stdout) handleOutput(childProcess.stdout, "stdout");
  if (childProcess.stderr) handleOutput(childProcess.stderr, "stderr");

  // Track if we've already stopped
  let stopped = false;

  const stop = async (): Promise<void> => {
    if (stopped) return;
    stopped = true;

    // Close log stream
    try {
      logStream.end();
    } catch {
      // Ignore
    }

    // Kill the process
    if (childProcess.pid && !childProcess.killed) {
      try {
        // Try graceful shutdown first
        childProcess.kill("SIGTERM");

        // Wait for process to exit (with timeout)
        await Promise.race([
          new Promise<void>((resolve) => {
            childProcess.on("exit", () => resolve());
          }),
          sleep(5000),
        ]);

        // Force kill if still running
        if (!childProcess.killed) {
          childProcess.kill("SIGKILL");
        }
      } catch {
        // Best effort
      }
    }

    // Clean up data directory (only if we created it, not if externally provided)
    if (!options.keepDataDir && !isExternalDataDir) {
      removeDataDir(dataDir);
    }
  };

  // Handle unexpected process exit
  childProcess.on("error", (err) => {
    console.error(`[${options.name}] Process error:`, err);
  });

  childProcess.on("exit", (code, signal) => {
    if (!stopped) {
      console.log(
        `[${options.name}] Process exited unexpectedly: code=${code}, signal=${signal}`,
      );
    }
  });

  // Wait for server to be ready
  const healthCheckUrl = options.healthCheckPath
    ? `${baseUrl}${options.healthCheckPath}`
    : baseUrl;
  try {
    await waitForHttpOk(healthCheckUrl, { timeoutMs: 60_000, intervalMs: 500 });
  } catch (e) {
    // Server failed to start - clean up and rethrow
    await stop();
    throw new Error(
      `Failed to start Kellnr at ${baseUrl}: ${(e as Error).message}. ` +
        `Check logs at ${logFile}`,
    );
  }

  return {
    process: childProcess,
    baseUrl,
    dataDir,
    logFile,
    stop,
  };
}

/**
 * Default configuration for local Kellnr tests.
 * Mirrors the kellnrDefaults() function from kellnr.ts.
 */
export function localKellnrDefaults(options?: {
  baseUrl?: string;
  logLevel?: string;
  webLogLevel?: string;
}): {
  baseUrl: string;
  port: number;
  env: Record<string, string>;
} {
  const port = 8000;
  const baseUrl =
    options?.baseUrl ?? process.env.KELLNR_BASE_URL ?? `http://localhost:${port}`;
  const logLevel = options?.logLevel ?? "debug";
  const webLogLevel = options?.webLogLevel ?? "debug";

  return {
    baseUrl,
    port,
    env: {
      KELLNR_LOG__LEVEL: logLevel,
      KELLNR_LOG__LEVEL_WEB_SERVER: webLogLevel,
      KELLNR_ORIGIN__PORT: String(port),
    },
  };
}
