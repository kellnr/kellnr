<template>
  <header class="k-header">
      <div id="k-logo" class="k-header-element">
        <router-link class="k-header-element-link" to="/">
          <span id="kellnrLogo">&lt;'kellnr&gt;</span>
        </router-link>
      </div>

      <div id="k-link" class="k-header-links">
        <div class="k-header-element">
          <router-link class="k-header-element-link" to="/crates">
            <div class="k-header-element-icon"><i class="fas fa-magnifying-glass"></i></div>
            <div class="k-header-element-text">Search</div>
          </router-link>
        </div>

        <div class="k-header-element" v-on:click="login()">
          <div class="k-header-element-link">
            <div class="k-header-element-icon"><i class="fas fa-gear"></i></div>
            <div class="k-header-element-text">Settings</div>
          </div>
        </div>

        <div class="k-header-element">
          <router-link to="/docqueue" class="k-header-element-link">
            <div class="k-header-element-icon"><i class="fas fa-layer-group"></i></div>
            <div class="k-header-element-text">Doc Queue</div>
          </router-link>
        </div>

        <div class="k-header-element">
          <a
              href="https://kellnr.io/documentation"
              target="_blank"
              class="k-header-element-link"
          >
            <div class="k-header-element-icon"><i class="fas fa-book"></i></div>
            <div class="k-header-element-text">Help</div>
          </a>
        </div>
      </div>

      <div id="k-button" class="k-header-buttons">
        <div class="k-header-element">
          <div class="k-button" id="toggleTheme" v-on:click="toggleTheme">
          <span class="icon">
            <i class="fas fa-adjust"></i>
          </span>
          </div>
        </div>

        <div id="login-button" class="k-header-element">
          <login-button></login-button>
        </div>
      </div>
  </header>
</template>

<script setup lang="ts">
import LoginButton from "./LoginButton.vue";
import {onBeforeMount} from "vue";
import {MutationTypes} from "../store/mutation-types";
import {store} from "../store/store";
import router from "../router";

onBeforeMount(() => {
  const theme = store.state.theme
  setTheme(theme)
})

function login() {
  if(store.state.loggedIn === false) {
    router.push("/login?redirect=settings")
  } else {
    router.push("/settings")
  }
}

function toggleTheme() {
  store.commit(MutationTypes.TOGGLE_THEME, null);
  const theme = store.state.theme;
  setTheme(theme);
}

function setTheme(theme: string) {
  let body = document.getElementById("body");
  body?.setAttribute("color-theme", theme);
}
</script>

<style scoped>
.k-header {
  display: grid;
  padding: 0 1rem;
}

#k-logo {
  grid-area: logo;
}

#k-link {
  grid-area: link;
  display: flex;
}

#k-button {
  grid-area: button;
}

#toggleTheme {
  margin-right: 0.5rem;
}

body[color-theme="dark"] .k-header {
  border-bottom-color: var(--dark-color-middle);
}

#kellnrLogo {
  color: var(--color-dark);
}

body[color-theme="dark"] #kellnrLogo {
  background: linear-gradient(to right, var(--dark-color-middle) 0%, var(--dark-color-dark) 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.k-header-links {
  display: flex;
  justify-content: center;
  align-items: center;
}

.k-header-buttons {
  display: flex;
  justify-content: flex-end;
  align-items: center;
}

.k-header-element {
  cursor: pointer;
  font-weight: bolder;
}

.k-header-element-link {
  color: var(--color-darkest);
}

.k-header-element-link:hover {
  color: var(--color-middle);
}

.k-header-element-icon {
  display: flex;
  justify-content: center;
  align-items: center;
  margin-bottom: 0.2rem;
}

.k-header-element-text {
  text-transform: uppercase;
  letter-spacing: 0.8px;
}

body[color-theme="dark"] .k-header-element-link {
  color: var(--dark-color-middle);
}

body[color-theme="dark"] .k-header-element-link:hover {
  color: var(--dark-color-dark);
}

@media (max-width: 768px) {
  .k-header {
    grid-template-columns: 1fr 1fr;
    grid-template-rows: auto auto;
    grid-template-areas: "logo button" "link link";
    row-gap: 1rem;;
    /* padding-bottom: 1rem; */
  }
  
  #kellnrLogo {
    font-size: 2.125rem;
  }

  #k-link {
    gap: 1rem;
  }

  .k-header-element-icon {
    font-size: 0.8rem;
  }

  .k-header-element-text {
    font-size: 0.8rem;
  }

  .k-header-buttons{
    margin-top: 0.625rem;
    height: 2rem;
  }
}

@media (min-width: 768px) {
  .k-header {
    grid-template-columns: auto 1fr auto;
    grid-template-areas: "logo link button"
    /* padding-bottom: 1rem; */
  }

  #kellnrLogo {
    font-size: 2.125rem;
  }

  #k-link {
    gap: 2rem;
  }

  .k-header-element-icon {
    font-size: 1.2rem;
  }
  .k-header-element-text {
    font-size: 1rem;
  }
}
</style>
