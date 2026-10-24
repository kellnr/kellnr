import { OAUTH2_LOGIN } from "../remote-routes";

// The SSO login leaves the app and comes back with a full page load, so the
// app cannot tell a fresh login from a restored session without this marker.
const SSO_LOGIN_PENDING_KEY = "kellnr-sso-login-pending";

/**
 * Redirect to the OAuth2 provider and remember that a login was started.
 */
export function startSsoLogin(): void {
  try {
    sessionStorage.setItem(SSO_LOGIN_PENDING_KEY, "1");
  } catch {
    // Without storage the login still works, only the welcome message is skipped
  }
  window.location.href = OAUTH2_LOGIN;
}

/**
 * Return whether an SSO login was started in this tab and clear the marker.
 */
export function takeSsoLoginPending(): boolean {
  try {
    const pending = sessionStorage.getItem(SSO_LOGIN_PENDING_KEY) === "1";
    sessionStorage.removeItem(SSO_LOGIN_PENDING_KEY);
    return pending;
  } catch {
    return false;
  }
}
