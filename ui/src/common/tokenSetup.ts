import { REGISTRY_NAME } from "./registry.ts";

/**
 * Command that stores a token for the Kellnr registry.
 *
 * Cargo asks for the token on stdin, so it does not end up in the shell history.
 */
export function cargoLoginCommand(): string {
  return `cargo login --registry ${REGISTRY_NAME}`;
}
