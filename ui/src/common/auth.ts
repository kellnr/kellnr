import {useRouter} from "vue-router";
import {VERSION} from "../remote-routes";
import {store} from "../store/store";
import axios from "axios"

export function login_required() {
    const router = useRouter()
  if(store.state.loggedIn === false) {
    // Check if authentication is required
    // to view crates. -> "auth_required = true" in Kellnr settings.
    
    axios.get(VERSION).then((_response) => {
      // do nothing -> no auth required
    }).catch((error) => {
      if(error.response.status === 401) {
        router.push("/login")
      }
    })
  } 
}
