/**
 * Thin helpers for Kellnr smoke tests.
 *
 * Goal: keep the "fixed localhost:8000" convention in one place, so specs stay small
 * and consistent without introducing a DSL.
 */

import process from "node:process";
import type { TestInfo } from "@playwright/test";
import type { PortBindings } from "./docker";
import type { Started, StartedNetwork } from "./docker";
import { startContainer, withStartedContainer } from "./docker";

export type KellnrDefaults = {
  /**
   * Fixed host port mapping used by these tests.
   * Container port 8000 is mapped to host port 8000.
   */
  ports: PortBindings;

  /**
   * Base URL matching the fixed port mapping.
   */
  baseUrl: string;

  /**
   * Environment variables that should be applied to *every* Kellnr container in tests.
   *
   * Tests can extend or override these by spreading:
   *   env: { ...k.env, KELLNR_PROXY__ENABLED: "true" }
   */
  env: Record<string, string>;
};

/**
 * Default config used across tests.
 *
 * Why fixed 8000:
 * - Existing test crates have `.cargo/config.toml` pointing at localhost:8000
 * - Kellnr generates stable URLs (including crates.io proxy download URLs) that embed the origin port
 * - Simplicity wins over parallelism for now
 */
export function kellnrDefaults(options?: {
  /**
   * Override the baseUrl (rare). If omitted, derives from the fixed port.
   */
  baseUrl?: string;

  /**
   * Change the log level defaults. Mostly useful while debugging locally.
   */
  logLevel?: string;
  webLogLevel?: string;
}): KellnrDefaults {
  const hostPort = 8000;

  const baseUrl =
    options?.baseUrl ??
    process.env.KELLNR_BASE_URL ??
    `http://localhost:${hostPort}`;

  const logLevel = options?.logLevel ?? "debug";
  const webLogLevel = options?.webLogLevel ?? "debug";

  return {
    ports: { 8000: hostPort },
    baseUrl,
    env: {
      KELLNR_LOG__LEVEL: logLevel,
      KELLNR_LOG__LEVEL_WEB_SERVER: webLogLevel,

      // Ensure Kellnr generates URLs with localhost:8000 (cratesio proxy download URLs, etc.)
      KELLNR_ORIGIN__PORT: String(hostPort),
    },
  };
}

export type StartKellnrOptions = {
  /**
   * Name prefix used for the container name. A unique suffix is added by docker helpers.
   */
  name: string;

  /**
   * Docker image to run.
   * Defaults to `KELLNR_TEST_IMAGE` or `kellnr-test:local`.
   */
  image?: string;

  /**
   * Extra environment variables applied on top of `kellnrDefaults().env`.
   */
  env?: Record<string, string>;

  /**
   * Bind mounts (hostPath -> containerPath).
   */
  bindMounts?: Record<string, string>;

  /**
   * Attach the container to a started docker network (e.g. S3/MinIO tests).
   */
  network?: StartedNetwork;

  /**
   * Optional docker network aliases.
   */
  networkAliases?: string[];

  /**
   * Extra labels for debugging/housekeeping.
   */
  labels?: Record<string, string>;

  /**
   * Override default baseUrl/log-level defaults (rare).
   */
  defaults?: Parameters<typeof kellnrDefaults>[0];
};

export type StartedKellnr = {
  started: Started;
  baseUrl: string;
};

export async function startKellnr(
  options: StartKellnrOptions,
  testInfo: TestInfo,
): Promise<StartedKellnr> {
  const k = kellnrDefaults(options.defaults);

  const image =
    options.image ?? process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";

  const started = await startContainer(
    {
      name: options.name,
      image,
      ports: k.ports,
      env: {
        ...k.env,
        ...(options.env ?? {}),
      },
      bindMounts: options.bindMounts,
      network: options.network,
      networkAliases: options.networkAliases,
      labels: options.labels,
    },
    testInfo,
  );

  return { started, baseUrl: k.baseUrl };
}

export async function withStartedKellnr<T>(
  testInfo: TestInfo,
  startedKellnr: StartedKellnr,
  fn: (ctx: { baseUrl: string }) => Promise<T>,
  opts?: { alwaysCollectLogs?: boolean },
): Promise<T> {
  return await withStartedContainer(
    testInfo,
    startedKellnr.started,
    async () => await fn({ baseUrl: startedKellnr.baseUrl }),
    opts,
  );
}
