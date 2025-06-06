<template>
  <div>
    <!-- App Bar -->
    <v-app-bar color="surface" flat>
      <!-- Mobile menu toggle -->
      <v-app-bar-nav-icon class="d-md-none" @click="drawer = !drawer"></v-app-bar-nav-icon>

      <!-- Logo -->
      <v-app-bar-title class="font-weight-bold text-h5">
        <router-link to="/" class="text-decoration-none">
          <span class="text-primary">&lt;'kellnr&gt;</span>
        </router-link>
      </v-app-bar-title>

      <!-- Desktop Navigation Links -->
      <div class="d-none d-md-flex ml-4">
        <v-btn v-for="(item, i) in navItems" :key="i" :to="item.route || undefined" :href="item.href || undefined"
          :target="item.href ? '_blank' : undefined" :ripple="false" variant="text" class="mx-2"
          :prepend-icon="item.icon" @click="item.action ? item.action() : null">
          {{ item.title }}
        </v-btn>
      </div>

      <v-spacer></v-spacer>

      <!-- Theme Toggle Component -->
      <ThemeToggle @theme-changed="handleThemeChange" />

      <!-- Login Button -->
      <span class="pr-4"><login-button></login-button></span>
    </v-app-bar>

    <!-- Mobile Navigation Drawer -->
    <v-navigation-drawer v-model="drawer" temporary location="left">
      <v-list>
        <v-list-item v-for="(item, i) in navItems" :key="i" :to="item.route || undefined" :href="item.href || undefined"
          :target="item.href ? '_blank' : undefined" :prepend-icon="item.icon"
          @click="item.action ? item.action() : null">
          <v-list-item-title>{{ item.title }}</v-list-item-title>
        </v-list-item>
      </v-list>
    </v-navigation-drawer>

    <!-- Snackbar for notifications -->
    <v-snackbar v-model="showSnackbar" :color="snackbarColor" :timeout="3000" location="bottom">
      {{ snackbarText }}
      <template v-slot:actions>
        <v-btn variant="text" icon="mdi-close" @click="showSnackbar = false" size="small"></v-btn>
      </template>
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { ref, onBeforeMount, computed } from "vue";
import LoginButton from "./LoginButton.vue";
import ThemeToggle from "./ThemeToggle.vue"; // Import the ThemeToggle component
import { useStore } from "../store/store";
import router from "../router";
import { useTheme } from "vuetify";

const store = useStore();
const vuetifyTheme = useTheme();
const drawer = ref(false);

// Snackbar state
const showSnackbar = ref(false);
const snackbarText = ref("");
const snackbarColor = ref("success");

// Navigation items
const navItems = computed(() => [
  {
    title: "Search",
    icon: "mdi-magnify",
    route: "/crates"
  },
  {
    title: "Settings",
    icon: "mdi-cog",
    action: login
  },
  {
    title: "Doc Queue",
    icon: "mdi-layers",
    route: "/docqueue"
  },
  {
    title: "Help",
    icon: "mdi-help-circle",
    href: "https://kellnr.io/documentation"
  }
]);

onBeforeMount(() => {
  // Apply the current theme
  vuetifyTheme.global.name.value = store.theme;
  setTheme(store.theme);
});

function login() {
  if (store.loggedIn === false) {
    router.push("/login?redirect=settings");
  } else {
    router.push("/settings");
  }
}

function handleThemeChange() {
  // Show notification for theme change
  showNotification(`Switched to ${store.theme} theme`);
}

function setTheme(theme: string) {
  let body = document.getElementById("body");
  body?.setAttribute("color-theme", theme);
}

// Show notification snackbar
function showNotification(message: string, isError: boolean = false) {
  snackbarText.value = message;
  snackbarColor.value = isError ? "error" : "success";
  showSnackbar.value = true;
}
</script>

<style scoped>
/* Any additional custom styles can go here */
</style>
