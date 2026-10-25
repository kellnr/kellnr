/**
 * Name of the Kellnr registry in cargo's configuration.
 *
 * Matches the name used in the setup guide on kellnr.io/documentation,
 * so the snippets shown in the UI work with a registry set up from the guide.
 */
export const REGISTRY_NAME = "kellnr";

/** Dependency line for Cargo.toml */
export function cargoTomlDependency(crateName: string, version: string): string {
  return `${crateName} = { version = "${version}", registry = "${REGISTRY_NAME}" }`;
}

/** Command that adds the crate as a dependency */
export function cargoAddCommand(crateName: string): string {
  return `cargo add ${crateName} --registry ${REGISTRY_NAME}`;
}
