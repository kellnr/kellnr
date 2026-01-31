<template>
  <div>
    <!-- App Bar -->
    <v-app-bar class="app-header" elevation="0" height="64">
      <!-- Mobile menu toggle -->
      <v-app-bar-nav-icon class="d-md-none nav-toggle" @click="drawer = !drawer" />

      <!-- Logo -->
      <v-app-bar-title class="logo-title">
        <span class="logo-text">&lt;'kellnr&gt;</span>
      </v-app-bar-title>

      <!-- Desktop Navigation Links -->
      <nav class="d-none d-md-flex nav-links">
        <v-btn
          to="/"
          :ripple="false"
          variant="text"
          class="nav-btn"
          prepend-icon="mdi-home"
        >
          Home
        </v-btn>
        <v-btn
          v-for="(item, i) in navItems"
          :key="i"
          :to="item.action ? undefined : item.route"
          :href="item.href || undefined"
          :target="item.href ? '_blank' : undefined"
          :ripple="false"
          variant="text"
          class="nav-btn"
          :prepend-icon="item.icon"
          @click="item.action ? item.action() : undefined"
        >
          {{ item.title }}
        </v-btn>
      </nav>

      <v-spacer />

      <!-- Theme Toggle Component -->
      <ThemeToggle @theme-changed="handleThemeChange" />

      <!-- Login Button -->
      <div class="login-wrapper">
        <login-button />
      </div>
    </v-app-bar>

    <!-- Mobile Navigation Drawer -->
    <v-navigation-drawer v-model="drawer" temporary location="left" class="mobile-drawer">
      <div class="drawer-header">
        <span class="drawer-logo">&lt;'kellnr&gt;</span>
      </div>
      <v-divider />
      <v-list nav class="drawer-list">
        <v-list-item
          to="/"
          prepend-icon="mdi-home"
          class="drawer-item"
        >
          <v-list-item-title>Home</v-list-item-title>
        </v-list-item>
        <v-list-item
          v-for="(item, i) in navItems"
          :key="i"
          :to="item.action ? undefined : item.route"
          :href="item.href || undefined"
          :target="item.href ? '_blank' : undefined"
          :prepend-icon="item.icon"
          class="drawer-item"
          @click="item.action ? item.action() : undefined"
        >
          <v-list-item-title>{{ item.title }}</v-list-item-title>
        </v-list-item>
      </v-list>
    </v-navigation-drawer>

    <!-- Snackbar for notifications -->
    <v-snackbar v-model="showSnackbar" :color="snackbarColor" :timeout="3000" location="bottom">
      {{ snackbarText }}
      <template v-slot:actions>
        <v-btn variant="text" icon="mdi-close" @click="showSnackbar = false" size="small" />
      </template>
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { ref, onBeforeMount, computed } from "vue";
import LoginButton from "./LoginButton.vue";
import ThemeToggle from "./ThemeToggle.vue";
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
    action: goToSettings
  },
  {
    title: "Doc Queue",
    icon: "mdi-layers",
    route: "/docqueue"
  },
  {
    title: "API Docs",
    icon: "mdi-api",
    href: "/api/docs"
  }
]);

onBeforeMount(() => {
  // Apply the current theme
  vuetifyTheme.global.name.value = store.theme;
  setTheme(store.theme);
});

function goToSettings() {
  if (store.loggedIn === false) {
    router.push({ path: "/login", query: { redirect: "settings" } });
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
.app-header {
  background: rgb(var(--v-theme-surface)) !important;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.nav-toggle {
  color: rgb(var(--v-theme-on-surface));
}

.logo-title {
  flex: none;
  width: auto;
  min-width: auto;
}

.logo-text {
  font-size: 1.5rem;
  font-weight: 700;
  color: rgb(var(--v-theme-primary));
  letter-spacing: -0.5px;
}

.nav-links {
  margin-left: 24px;
  gap: 4px;
}

.nav-btn {
  font-weight: 500;
  font-size: 0.875rem;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: rgb(var(--v-theme-on-surface));
  padding: 0 16px;
  height: 40px;
  border-radius: 8px;
  transition: all 0.2s ease;
}

.nav-btn:hover {
  background: rgba(var(--v-theme-primary), 0.08);
  color: rgb(var(--v-theme-primary));
}

.nav-btn :deep(.v-icon) {
  font-size: 18px;
  opacity: 0.8;
}

.nav-btn:hover :deep(.v-icon) {
  opacity: 1;
}

.login-wrapper {
  padding-right: 16px;
}

/* Mobile Drawer */
.mobile-drawer {
  background: rgb(var(--v-theme-surface)) !important;
}

.drawer-header {
  padding: 20px 16px;
  background: rgba(var(--v-theme-primary), 0.05);
}

.drawer-logo {
  font-size: 1.25rem;
  font-weight: 700;
  color: rgb(var(--v-theme-primary));
}

.drawer-list {
  padding: 8px;
}

.drawer-item {
  border-radius: 8px;
  margin-bottom: 4px;
}

.drawer-item:hover {
  background: rgba(var(--v-theme-primary), 0.08);
}

.drawer-item :deep(.v-list-item__prepend .v-icon) {
  color: rgb(var(--v-theme-primary));
  opacity: 0.8;
}
</style>
