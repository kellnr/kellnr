/**
 * CLI tests for kellnr command-line interface.
 *
 * Tests:
 * - Help/usage output for various commands
 * - Version output
 * - Config show command
 * - Config init command
 * - Run command validation (missing data_dir)
 * - Run command with valid config starts server
 *
 * Note: Most tests don't require starting the server, making them fast.
 */

import fs from "fs";
import path from "path";
import os from "os";
import { spawn } from "child_process";

import { test, expect } from "@playwright/test";
import {
  exec,
  getKellnrBinaryPath,
  assertKellnrBinaryExists,
  waitForHttpOk,
} from "./testUtils";

test.describe("CLI Tests", () => {
  let kellnrBinary: string;
  let tempDir: string;

  test.beforeAll(() => {
    assertKellnrBinaryExists();
    kellnrBinary = getKellnrBinaryPath();
    console.log(`[setup] Using kellnr binary: ${kellnrBinary}`);
  });

  test.beforeEach(() => {
    // Create a unique temp directory for each test
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "kellnr-cli-test-"));
  });

  test.afterEach(() => {
    // Clean up temp directory
    if (tempDir && fs.existsSync(tempDir)) {
      fs.rmSync(tempDir, { recursive: true, force: true });
    }
  });

  test.describe("Help and Version", () => {
    test("kellnr without arguments shows usage", async () => {
      const result = await exec(kellnrBinary, []);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("Usage:");
      expect(result.stdout).toContain("kellnr");
      expect(result.stdout).toContain("Commands:");
      expect(result.stdout).toContain("run");
      expect(result.stdout).toContain("config");
    });

    test("kellnr --help shows help", async () => {
      const result = await exec(kellnrBinary, ["--help"]);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("Usage:");
      expect(result.stdout).toContain("Options:");
      expect(result.stdout).toContain("--config");
      expect(result.stdout).toContain("--help");
      expect(result.stdout).toContain("--version");
    });

    test("kellnr --version shows version", async () => {
      const result = await exec(kellnrBinary, ["--version"]);

      expect(result.exitCode).toBe(0);
      // Version output should contain "kellnr" and a version number
      expect(result.stdout).toMatch(/kellnr\s+\d+\.\d+\.\d+/);
    });

    test("kellnr run --help shows run options", async () => {
      const result = await exec(kellnrBinary, ["run", "--help"]);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("Run the kellnr server");
      expect(result.stdout).toContain("--registry-data-dir");
      expect(result.stdout).toContain("--local-port");
      expect(result.stdout).toContain("--log-level");
    });

    test("kellnr config --help shows config subcommands", async () => {
      const result = await exec(kellnrBinary, ["config", "--help"]);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("Configuration management");
      expect(result.stdout).toContain("show");
      expect(result.stdout).toContain("init");
    });
  });

  test.describe("Config Commands", () => {
    test("kellnr config show outputs valid TOML", async () => {
      const result = await exec(kellnrBinary, ["config", "show"]);

      expect(result.exitCode).toBe(0);
      // Should contain TOML sections
      expect(result.stdout).toContain("[local]");
      expect(result.stdout).toContain("[registry]");
      expect(result.stdout).toContain("[docs]");
      expect(result.stdout).toContain("[proxy]");
      expect(result.stdout).toContain("[log]");
    });

    test("kellnr config init creates default config file", async () => {
      const configPath = path.join(tempDir, "kellnr.toml");

      const result = await exec(kellnrBinary, [
        "config",
        "init",
        "-o",
        configPath,
      ]);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("Configuration file created");
      expect(fs.existsSync(configPath)).toBe(true);

      // Verify file contains valid TOML config
      const content = fs.readFileSync(configPath, "utf-8");
      expect(content).toContain("[local]");
      expect(content).toContain("[registry]");
    });

    test("kellnr config init with default path creates kellnr.toml", async () => {
      // Run from temp directory so it creates kellnr.toml there
      const result = await exec(kellnrBinary, ["config", "init"], {
        cwd: tempDir,
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("Configuration file created");

      const configPath = path.join(tempDir, "kellnr.toml");
      expect(fs.existsSync(configPath)).toBe(true);
    });

    test("kellnr config init fails if file already exists", async () => {
      const configPath = path.join(tempDir, "existing.toml");

      // Create the file first
      fs.writeFileSync(configPath, "# existing config\n");

      const result = await exec(kellnrBinary, [
        "config",
        "init",
        "-o",
        configPath,
      ]);

      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("File already exists");
    });
  });

  test.describe("Run Command Validation", () => {
    test("kellnr run without data_dir shows helpful error", async () => {
      const result = await exec(kellnrBinary, ["run"]);

      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("No data directory configured");
      expect(result.stderr).toContain("--registry-data-dir");
      expect(result.stderr).toContain("KELLNR_REGISTRY__DATA_DIR");
    });

    test("kellnr run with -d short flag is recognized", async () => {
      // This should fail with a different error (port binding) not "unknown flag"
      // or succeed in starting. We just verify -d is a valid flag.
      const result = await exec(kellnrBinary, ["run", "-d", tempDir], {
        timeout: 5000,
      });

      // Should not complain about unknown flag
      expect(result.stderr).not.toContain("unexpected argument");
      expect(result.stderr).not.toContain("unknown");
    });
  });

  test.describe("Config File Loading", () => {
    test("kellnr config show with -c loads specified config file", async () => {
      // Create a custom config file
      const configPath = path.join(tempDir, "custom.toml");
      const customConfig = `
[local]
port = 9999

[registry]
data_dir = "/custom/path"
`;
      fs.writeFileSync(configPath, customConfig);

      const result = await exec(kellnrBinary, [
        "-c",
        configPath,
        "config",
        "show",
      ]);

      expect(result.exitCode).toBe(0);
      // Should show the custom values
      expect(result.stdout).toContain("9999");
      expect(result.stdout).toContain("/custom/path");
    });

    test("kellnr with non-existent config file shows error", async () => {
      const result = await exec(kellnrBinary, [
        "-c",
        "/nonexistent/path/kellnr.toml",
        "config",
        "show",
      ]);

      // Should exit with non-zero code
      expect(result.exitCode).not.toBe(0);
      // Should indicate config file issue
      expect(result.stderr.toLowerCase()).toMatch(/config|file|not found|error/);
    });
  });

  test.describe("Server Startup", () => {
    test("kellnr run starts server and responds to HTTP", async () => {
      // Use a unique port to avoid conflicts with other tests
      const port = 18080 + Math.floor(Math.random() * 1000);
      const dataDir = path.join(tempDir, "data");
      fs.mkdirSync(dataDir, { recursive: true });

      // Start kellnr in background
      const kellnrProcess = spawn(
        kellnrBinary,
        ["run", "-d", dataDir, "--local-port", String(port)],
        {
          stdio: ["ignore", "pipe", "pipe"],
          detached: true,
        }
      );

      kellnrProcess.stdout?.on("data", (data: Buffer) => {
        console.log(`[kellnr stdout] ${data.toString()}`);
      });
      kellnrProcess.stderr?.on("data", (data: Buffer) => {
        console.log(`[kellnr stderr] ${data.toString()}`);
      });

      try {
        // Wait for server to be ready
        await waitForHttpOk(`http://localhost:${port}/`, {
          timeoutMs: 30_000,
          intervalMs: 500,
        });

        // Server is up - verify it responds
        const response = await fetch(`http://localhost:${port}/`);
        expect(response.status).toBe(200);
      } finally {
        // Clean up: kill the process
        if (kellnrProcess.pid) {
          try {
            // Kill the process group (negative pid)
            process.kill(-kellnrProcess.pid, "SIGTERM");
          } catch {
            // Process may have already exited or not be a group leader
            try {
              process.kill(kellnrProcess.pid, "SIGTERM");
            } catch {
              // Ignore - process already gone
            }
          }
        }

        // Wait a bit for process to terminate
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
    });
  });
});
