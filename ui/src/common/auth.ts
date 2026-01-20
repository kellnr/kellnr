import { settingsService } from "../services"
import { isSuccess } from "../services/api"


export async function auth_required(): Promise<boolean> {
  // Check if authentication is required
  // to view crates. -> "auth_required = true" in Kellnr settings.
  const result = await settingsService.getVersion()

  if (isSuccess(result)) {
    // no auth required
    return false
  }

  // If we get a 401 or any error, assume auth is required
  return true
}
