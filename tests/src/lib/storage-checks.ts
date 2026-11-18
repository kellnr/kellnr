import { createHash } from "node:crypto";
import fs from "node:fs";
import { expect } from "@playwright/test";
import { DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./ui-fixtures";

/**
 * Checks shared by the S3 and GCS specs. They go through the HTTP API, as the
 * UI only reads the database and never the object storage.
 */

function sha256(data: Uint8Array): string {
  return createHash("sha256").update(data).digest("hex");
}

/**
 * Logs in as the default admin and returns the session cookie header value.
 */
export async function loginAsAdminViaApi(baseUrl: string): Promise<string> {
  const response = await fetch(`${baseUrl}/api/v1/auth/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ user: DEFAULT_ADMIN_USER, pwd: DEFAULT_ADMIN_PASSWORD }),
  });
  if (!response.ok) {
    throw new Error(`Login as ${DEFAULT_ADMIN_USER} failed: HTTP ${response.status} ${await response.text()}`);
  }
  const session = response.headers.get("set-cookie")?.match(/kellnr_session_id=([^;]+)/);
  if (!session) {
    throw new Error(`Login as ${DEFAULT_ADMIN_USER} returned no kellnr_session_id cookie`);
  }
  return `kellnr_session_id=${session[1]}`;
}

/**
 * Path of a crate in the sparse index, for names with four or more characters.
 */
function sparseIndexPath(name: string): string {
  if (name.length < 4) {
    throw new Error(`sparseIndexPath supports names with 4 or more characters, got ${name}`);
  }
  const lower = name.toLowerCase();
  return `${lower.slice(0, 2)}/${lower.slice(2, 4)}/${lower}`;
}

/**
 * Downloads a crate through the registry API and compares its checksum with
 * the one the sparse index recorded when the crate was published.
 */
export async function expectCrateDownloadMatchesIndex(
  baseUrl: string,
  crate: { name: string; version: string },
): Promise<void> {
  const indexResponse = await fetch(`${baseUrl}/api/v1/crates/${sparseIndexPath(crate.name)}`);
  expect(indexResponse.status, `index entry of ${crate.name}`).toBe(200);
  const entry = (await indexResponse.text())
    .split("\n")
    .filter((line) => line.trim().length > 0)
    .map((line) => JSON.parse(line) as { vers: string; cksum: string })
    .find((version) => version.vers === crate.version);
  if (!entry) {
    throw new Error(`The index of ${crate.name} has no version ${crate.version}`);
  }

  const response = await fetch(`${baseUrl}/api/v1/crates/dl/${crate.name}/${crate.version}/download`);
  expect(response.status, `download of ${crate.name} ${crate.version}`).toBe(200);
  const downloaded = new Uint8Array(await response.arrayBuffer());
  expect(sha256(downloaded), `checksum of ${crate.name} ${crate.version}`).toBe(entry.cksum);
}

/**
 * Crates the storage specs publish, including a name with upper-case letters,
 * which the storage key keeps.
 */
export const PUBLISHED_STORAGE_TEST_CRATES = [
  { name: "test_lib", version: "1.0.0" },
  { name: "UpperCase-Name123", version: "0.1.0" },
  { name: "foo-bar", version: "1.0.0" },
];

const TEST_TOOLCHAIN_VERSION = "1.0.0-test";

export interface ToolchainUpload {
  target: string;
  archivePath: string;
}

/**
 * Uploads a toolchain archive for one target of the test toolchain on the stable channel.
 */
export async function uploadToolchain(baseUrl: string, cookie: string, upload: ToolchainUpload): Promise<void> {
  const url = `${baseUrl}/api/v1/toolchains?name=rust&version=${TEST_TOOLCHAIN_VERSION}&target=${upload.target}&date=2024-01-15&channel=stable`;
  const response = await fetch(url, {
    method: "PUT",
    headers: { "Content-Type": "application/octet-stream", Cookie: cookie },
    body: fs.readFileSync(upload.archivePath),
  });
  expect(response.status, `upload of ${upload.target}: ${response.status === 200 ? "" : await response.text()}`).toBe(200);
  expect((await response.json()).success).toBe(true);
}

/**
 * Waits until the stable manifest lists all targets, which happens once their
 * archives were split into components in the storage, and returns it.
 */
export async function waitForStableManifest(baseUrl: string, cookie: string, targets: string[]): Promise<string> {
  const url = `${baseUrl}/api/v1/toolchains/dist/channel-rust-stable.toml`;
  let manifest = "";
  await expect
    .poll(
      async () => {
        const response = await fetch(url, { headers: { Cookie: cookie } });
        manifest = response.ok ? await response.text() : "";
        return targets.filter((target) => !manifest.includes(target));
      },
      { message: `targets missing from ${url}`, timeout: 60_000, intervals: [200] },
    )
    .toEqual([]);
  return manifest;
}

/**
 * Returns the URL of a target's archive from the manifest: the uploaded
 * archive for "rust", or one of the components split from it.
 */
export function toolchainArchiveUrl(manifest: string, target: string, component: string = "rust"): string {
  const fileName = `${component}-${TEST_TOOLCHAIN_VERSION}-${target}.tar.xz`;
  const url = [...manifest.matchAll(/xz_url = "([^"]+)"/g)]
    .map((match) => match[1])
    .find((candidate) => candidate.endsWith(`/${fileName}`));
  if (!url) {
    throw new Error(`The manifest has no xz_url for ${fileName}:\n${manifest}`);
  }
  return url;
}

/**
 * Downloads a toolchain archive and compares it with the uploaded file.
 */
export async function expectToolchainDownloadMatches(url: string, cookie: string, archivePath: string): Promise<void> {
  const response = await fetch(url, { headers: { Cookie: cookie } });
  expect(response.status, `download of ${url}`).toBe(200);
  expect(response.headers.get("content-type")).toBe("application/x-xz");
  const downloaded = new Uint8Array(await response.arrayBuffer());
  expect(sha256(downloaded), `checksum of ${url}`).toBe(sha256(fs.readFileSync(archivePath)));
}

/**
 * Deletes one target of the test toolchain.
 */
export async function deleteToolchainTarget(baseUrl: string, cookie: string, target: string): Promise<void> {
  const response = await fetch(`${baseUrl}/api/v1/toolchains/rust/${TEST_TOOLCHAIN_VERSION}/targets/${target}`, {
    method: "DELETE",
    headers: { Cookie: cookie },
  });
  expect(response.status, `delete of target ${target}`).toBe(200);
  expect((await response.json()).success).toBe(true);
}

/**
 * Expects a download URL to return the given status.
 */
export async function expectDownloadStatus(url: string, cookie: string, status: number): Promise<void> {
  const response = await fetch(url, { headers: { Cookie: cookie } });
  expect(response.status, `download of ${url}`).toBe(status);
}
