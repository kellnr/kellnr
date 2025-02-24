import { useRouter } from "vue-router";
import { VERSION } from "../remote-routes";
import { useStore } from "../store/store";
import axios from "axios"


export function login_required() {
  const router = useRouter()
  const store = useStore();

  if (store.loggedIn === false) {
    // Check if authentication is required
    // to view crates. -> "auth_required = true" in Kellnr settings.

    axios.get(VERSION).then(() => {
      // do nothing -> no auth required
    }).catch((error) => {
      if (error.response.status === 401) {
        router.push("/login")
      }
    })
  }
}
