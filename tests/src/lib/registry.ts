import fs from "node:fs";
import path from "node:path";

/**
 * Read the auth token for a given Cargo registry name from a crate-local `.cargo/config.toml`.
 *
 * Why:
 * - We want one source of truth for Cargo configuration (crate-local config).
 * - Cargo does NOT allow setting `registries.<name>.token` via `--config`, so tests need
 *   to read the token from config.toml and pass it via `CARGO_REGISTRIES_<NAME>_TOKEN`.
 *
 * Supported config formats (order of keys doesn't matter):
 * - Inline table:
 *     [registries]
 *     kellnr-test = { index = "http://localhost:8000/api/v1/crates/", token = "abc" }
 *
 * - Bracketed table:
 *     [registries.kellnr-test]
 *     index = "http://localhost:8000/api/v1/crates/"
 *     token = "abc"
 *
 * This helper intentionally does NOT parse TOML via a dependency to keep the test harness small
 * and easy to run. It uses conservative regex matching and provides actionable error messages.
 */
export function extractRegistryTokenFromCargoConfig(options: {
  /**
   * Absolute path to the crate directory that contains `.cargo/config.toml`.
   */
  crateDir: string;

  /**
   * Registry name as used by Cargo, e.g. "kellnr-test" or "kellnr-local".
   */
  registryName: string;
}): string {
  const configPath = path.resolve(options.crateDir, ".cargo", "config.toml");

  if (!fs.existsSync(configPath)) {
    throw new Error(
      `Cargo config not found: ${configPath}\n` +
        `Expected a crate-local .cargo/config.toml containing the registry token.`,
    );
  }

  const contents = fs.readFileSync(configPath, "utf8");
  const registryName = options.registryName;

  // Escape for use in RegExp.
  const escaped = registryName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

  // 1) Inline table form under [registries]
  //
  // Example:
  //   kellnr-test = { index = "...", token = "..." }
  //
  // We match token="..." inside the braces for the specific registry key.
  const inlineTokenRegex = new RegExp(
    String.raw`${escaped}\s*=\s*\{[^}]*\btoken\s*=\s*"([^"]+)"[^}]*\}`,
    "m",
  );
  const inlineMatch = contents.match(inlineTokenRegex);
  if (inlineMatch?.[1]) return inlineMatch[1];

  // 2) Bracketed table form:
  //
  // Example:
  //   [registries.kellnr-test]
  //   token = "..."
  //
  // We capture only within the section until the next [section] header.
  const sectionHeaderRegex = new RegExp(
    String.raw`^\[registries\.${escaped}\]\s*$`,
    "m",
  );
  const headerMatch = contents.match(sectionHeaderRegex);

  if (headerMatch) {
    const headerIndex = headerMatch.index ?? -1;
    if (headerIndex >= 0) {
      const afterHeader = contents.slice(headerIndex + headerMatch[0].length);

      // Stop at next section header or end of file.
      const nextSectionIdx = afterHeader.search(/^\s*\[[^\]]+\]\s*$/m);
      const sectionBody =
        nextSectionIdx >= 0 ? afterHeader.slice(0, nextSectionIdx) : afterHeader;

      const tokenLineMatch = sectionBody.match(
        /^\s*token\s*=\s*"([^"]+)"\s*$/m,
      );
      if (tokenLineMatch?.[1]) return tokenLineMatch[1];
    }
  }

  // Provide a helpful error with a quick peek at relevant lines.
  const previewLines = contents
    .split(/\r?\n/)
    .filter((l) => l.includes(registryName) || l.includes("registries") || l.includes("token"))
    .slice(0, 30)
    .join("\n");

  throw new Error(
    `Failed to extract Cargo registry token.\n` +
      `- registryName: ${registryName}\n` +
      `- configPath: ${configPath}\n\n` +
      `Expected one of the following:\n` +
      `1) Inline table:\n` +
      `   [registries]\n` +
      `   ${registryName} = { index = "...", token = "..." }\n\n` +
      `2) Bracketed table:\n` +
      `   [registries.${registryName}]\n` +
      `   token = "..."\n\n` +
      (previewLines
        ? `Relevant lines preview:\n${previewLines}\n`
        : `Config file contained no obvious registry/token lines.\n`),
  );
}
