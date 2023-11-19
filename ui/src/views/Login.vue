<template>
  <div class="login-page">
    <div class="glass login-form">
      <h1 class="k-h2 center-wrapper">Sign In</h1>
    <form>
      <div class="field">
        <div class="control is-expanded has-icons-left">
          <input
              class="input is-info"
              v-model="user"
              placeholder="User"
              type="text"
              required
          />
          <span class="icon is-small is-left">
          <i class="fas fa-user"></i>
        </span>
        </div>
      </div>
      <div class="field">
        <div class="control has-icons-left">
          <input
              class="input is-info"
              v-model="pwd"
              placeholder="Password"
              type="password"
              required
          />
          <span class="icon is-small is-left">
          <i class="fas fa-lock"></i>
        </span>
        </div>
      </div>

      <div id="remember">
        <input type="checkbox" id="remember-box" v-model="store.state.rememberMe" name="remember">
        <label for="remember" id="remember-label">Remember me</label>
      </div>

      <status-notification :status="loginStatus" @update:clear="loginStatus = $event">
        {{ loginStatusMsg }}
      </status-notification>

      <div class="center-wrapper">
        <button @click.prevent="submit()" class="button is-info login-button">Confirm</button>
      </div>

    </form>
    </div>
  </div>
</template>

<script setup lang="ts">
import StatusNotification from "../components/StatusNotification.vue";
import {onMounted, ref} from "vue";
import {MutationTypes} from "../store/mutation-types";
import axios from "axios";
import {store} from "../store/store"
import {LOGIN} from "../remote-routes";
import router from "../router";

const loginStatusMsg = ref("")
const loginStatus = ref("") // "", "Error", "Success"
const user = ref("")
const pwd = ref("")

onMounted(() => {
  if(store.state.rememberMe && store.state.rememberMeUser != "") {
    user.value = store.state.rememberMeUser;
  }
})

function submit() {
  const postData = {user: user.value, pwd: pwd.value};
  axios
    .post(LOGIN, postData)
    .then((res) => {
      if (res.status == 200) {
        loginStatusMsg.value = "Login successfull";
        loginStatus.value = "Success";
        store.commit(MutationTypes.LOGIN, res.data);
        if(store.state.rememberMe) {
          store.state.rememberMeUser = user.value;
        }
        if(router.currentRoute.value.query["redirect"] === "settings") {
          if(store.state.loggedInUserIsAdmin) {
            router.push("/adminsettings")
          } else {
            router.push("/usersettings")
          }
        }
        else {
          router.push("/")
        }
      }
    })
    .catch((error) => {
      if (error.response) {
        loginStatus.value = "Error";
        if (error.response.status == 401) {
          loginStatusMsg.value = "Wrong user or password";
        } else if (error.response.status == 500) {
          loginStatusMsg.value = "Internal server error";
        } else {
          loginStatusMsg.value = "Unknown error";
        }
      }
    });
}
</script>

<style scoped>
  .login-page {
    display: flex;
    justify-content: center;
    align-items: center;
  }
  .login-form {
    min-width: 30%;
  }

  #remember-box {
    margin-bottom: 1rem;
  }
  #remember-label {
    margin-left: 0.5rem;
    color: var(--color-darkest);
  }

  body[color-theme="dark"] #remember-label {
    color: var(--dark-color-white);
  }

  .center-wrapper {
    display: flex;
    justify-content: center;
  }

</style>
