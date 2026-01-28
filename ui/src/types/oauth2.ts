/**
 * OAuth2/OpenID Connect types
 */

/**
 * OAuth2 configuration returned by the backend
 */
export interface OAuth2Config {
  /** Whether OAuth2 authentication is enabled */
  enabled: boolean
  /** Text to display on the OAuth2 login button */
  button_text: string
}
