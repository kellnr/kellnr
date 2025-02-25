import { VERSION } from "../remote-routes";
import axios from "axios"


export async function auth_required(): Promise<boolean> {
  // Check if authentication is required
  // to view crates. -> "auth_required = true" in Kellnr settings.
  return axios.get(VERSION).then((_response) => {
    // no auth required
    return false
  }).catch((error) => {
    if (error.response.status === 401) {
      // auth required
      return true
    }
    else {
      // unknown error
      return true
    }
  })
}
