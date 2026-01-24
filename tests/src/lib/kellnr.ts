/**
 * Kellnr configuration helpers for tests.
 *
 * Provides default configuration values for the fixed localhost:8000 convention
 * used across all tests.
 */

import process from "node:process";
import type { PortBindings } from "./docker";

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
   * Environment variables that should be applied to every Kellnr instance in tests.
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
