/**
 * Mock OIDC server for OAuth2 testing.
 *
 * Uses the `ghcr.io/navikt/mock-oauth2-server` Docker image which provides
 * a lightweight, configurable OAuth2/OIDC server for testing purposes.
 *
 * Features:
 * - Automatic OIDC discovery endpoint
 * - Configurable token responses
 * - Support for PKCE
 * - Customizable claims
 */

import { GenericContainer, Wait, type StartedTestContainer } from "testcontainers";
import type { TestInfo } from "@playwright/test";
import type { BeforeAllTestInfo } from "../testUtils";

/**
 * Configuration for the mock OIDC server.
 */
export type MockOidcConfig = {
  /** The issuer URL (set after container starts) */
  issuerUrl: string;
  /** OAuth2 client ID */
  clientId: string;
  /** OAuth2 client secret */
  clientSecret: string;
  /** The host port the server is accessible on */
  port: number;
};

/**
 * Result of starting a mock OIDC server.
 */
export type StartedMockOidc = {
  container: StartedTestContainer;
  config: MockOidcConfig;
  /** Stop the container */
  stop: () => Promise<void>;
};

/**
 * Options for starting the mock OIDC server.
 */
export type StartMockOidcOptions = {
  /** Name prefix for the container */
  name?: string;
  /** Client ID to use (default: "kellnr") */
  clientId?: string;
  /** Client secret to use (default: "kellnr-secret") */
  clientSecret?: string;
};

/**
 * Start a mock OIDC server for testing OAuth2 flows.
 *
 * The server provides:
 * - OIDC discovery at /.well-known/openid-configuration
 * - Authorization endpoint
 * - Token endpoint
 * - JWKS endpoint
 *
 * @example
 * ```typescript
 * const oidc = await startMockOidcServer({ name: "test-oauth2" }, testInfo);
 * // Configure Kellnr with:
 * // - issuer_url: oidc.config.issuerUrl
 * // - client_id: oidc.config.clientId
 * // - client_secret: oidc.config.clientSecret
 * await oidc.stop();
 * ```
 */
export async function startMockOidcServer(
  options: StartMockOidcOptions = {},
  _testInfo?: TestInfo | BeforeAllTestInfo,
): Promise<StartedMockOidc> {
  const clientId = options.clientId ?? "kellnr";
  const clientSecret = options.clientSecret ?? "kellnr-secret";
  // Generate a unique name without relying on testInfo (which may be undefined in beforeAll)
  const baseName = options.name ?? "mock-oidc";
  const name = `${baseName}-${Date.now()}`;

  // The mock-oauth2-server image from NAV (Norwegian Labour and Welfare Administration)
  // is specifically designed for testing OAuth2/OIDC flows
  const container = await new GenericContainer("ghcr.io/navikt/mock-oauth2-server:2.1.10")
    .withName(name)
    .withExposedPorts(8080)
    .withEnvironment({
      // Configure the server to accept any client by default
      JSON_CONFIG: JSON.stringify({
        interactiveLogin: true,
        httpServer: "NettyWrapper",
        tokenCallbacks: [
          {
            issuerId: "default",
            tokenExpiry: 3600,
            requestMappings: [
              {
                requestParam: "grant_type",
                match: "*",
                claims: {
                  sub: "test-user",
                  email: "test@example.com",
                  preferred_username: "testuser",
                  groups: ["kellnr-users"],
                },
              },
            ],
          },
        ],
      }),
    })
    .withWaitStrategy(Wait.forHttp("/.well-known/openid-configuration", 8080).forStatusCode(200))
    .start();

  const port = container.getMappedPort(8080);
  const issuerUrl = `http://localhost:${port}/default`;

  const config: MockOidcConfig = {
    issuerUrl,
    clientId,
    clientSecret,
    port,
  };

  return {
    container,
    config,
    stop: async () => {
      await container.stop();
    },
  };
}

/**
 * Generate Kellnr environment variables for OAuth2 configuration.
 */
export function getOAuth2EnvVars(config: MockOidcConfig): Record<string, string> {
  return {
    KELLNR_OAUTH2__ENABLED: "true",
    KELLNR_OAUTH2__ISSUER_URL: config.issuerUrl,
    KELLNR_OAUTH2__CLIENT_ID: config.clientId,
    KELLNR_OAUTH2__CLIENT_SECRET: config.clientSecret,
    KELLNR_OAUTH2__AUTO_PROVISION_USERS: "true",
    KELLNR_OAUTH2__BUTTON_TEXT: "Login with SSO",
  };
}

/**
 * Generate Kellnr environment variables for OAuth2 with admin group.
 */
export function getOAuth2EnvVarsWithAdmin(
  config: MockOidcConfig,
  adminGroup: string = "kellnr-admins",
): Record<string, string> {
  return {
    ...getOAuth2EnvVars(config),
    KELLNR_OAUTH2__ADMIN_GROUP_CLAIM: "groups",
    KELLNR_OAUTH2__ADMIN_GROUP_VALUE: adminGroup,
  };
}
