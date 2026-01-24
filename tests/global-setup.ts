import type { FullConfig } from "@playwright/test";
import fs from "node:fs";
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

function repoRootFromTests(): string {
    // global-setup.ts lives in tests/. When invoked by Playwright, process.cwd()
    // is typically tests/, but we keep this robust.
    return path.resolve(process.cwd(), "..");
}

/**
 * Check if the Kellnr binary exists and is recent enough.
 * Returns true if we should skip rebuilding.
 */
function binaryExistsAndRecent(binaryPath: string, maxAgeMs: number = 0): boolean {
    try {
        const stats = fs.statSync(binaryPath);
        if (maxAgeMs === 0) {
            return true;
        }
        const age = Date.now() - stats.mtimeMs;
        return age < maxAgeMs;
    } catch {
        return false;
    }
}

/**
 * Playwright globalSetup:
 * - Build Kellnr locally once per *entire* test run (not per worker).
 * - Compiles the frontend (just npm-build) and Rust backend (just build).
 *
 * This replaces the previous Docker-based approach for faster iteration.
 * Docker is still used for:
 * - S3 tests (MinIO container)
 * - Migration tests (old Kellnr version in Docker)
 */
export default async function globalSetup(_config: FullConfig) {
    const repoRoot = repoRootFromTests();
    const binaryPath = path.resolve(repoRoot, "target", "debug", "kellnr");

    // Allow skipping build via environment variable
    // Useful when running tests repeatedly during development
    if (process.env.KELLNR_SKIP_BUILD === "1") {
        // eslint-disable-next-line no-console
        console.log("[globalSetup] KELLNR_SKIP_BUILD=1, skipping build");
        if (!binaryExistsAndRecent(binaryPath)) {
            throw new Error(
                `KELLNR_SKIP_BUILD=1 but binary not found at ${binaryPath}. ` +
                    `Run 'just npm-build && just build' first.`,
            );
        }
        return;
    }

    // Check if we should force rebuild
    const forceRebuild = process.env.KELLNR_FORCE_REBUILD === "1";

    // If binary exists and we're not forcing rebuild, skip
    if (!forceRebuild && binaryExistsAndRecent(binaryPath)) {
        // eslint-disable-next-line no-console
        console.log(`[globalSetup] Binary exists at ${binaryPath}`);
        // eslint-disable-next-line no-console
        console.log("[globalSetup] Skipping build (set KELLNR_FORCE_REBUILD=1 to force)");
        return;
    }

    // Check if 'just' is available
    try {
        await execOrThrow("just", ["--version"]);
    } catch {
        throw new Error(
            "The 'just' command runner is required but not found. " +
                "Install it via: cargo install just",
        );
    }

    // Build the frontend
    // eslint-disable-next-line no-console
    console.log("[globalSetup] Building frontend (just npm-build)...");
    await execOrThrow("just", ["npm-build"], { cwd: repoRoot });
    // eslint-disable-next-line no-console
    console.log("[globalSetup] Frontend built successfully");

    // Build the backend
    // eslint-disable-next-line no-console
    console.log("[globalSetup] Building backend (just build)...");
    await execOrThrow("just", ["build"], { cwd: repoRoot });
    // eslint-disable-next-line no-console
    console.log("[globalSetup] Backend built successfully");

    // Verify binary was created
    if (!binaryExistsAndRecent(binaryPath)) {
        throw new Error(
            `Build completed but binary not found at ${binaryPath}. ` +
                `Check the build output for errors.`,
        );
    }

    // eslint-disable-next-line no-console
    console.log(`[globalSetup] Kellnr binary ready at ${binaryPath}`);
}
