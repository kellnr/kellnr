import { createRouter, createWebHistory } from 'vue-router'
import Crates from "../views/Crates.vue";
import Login from "../views/Login.vue";
import Settings from "../views/Settings.vue";
import PublishDocs from "../views/PublishDocs.vue";
import Crate from "../views/Crate.vue";
import DocQueue from "../views/DocQueue.vue";
import Landing from "../views/Landing.vue";
import { auth_required } from "../common/auth";
import { useStore } from "../store/store";

const routes = [
  {
    path: '/',
    name: 'Landing',
    component: Landing,
    meta: {
      requiresAuth: true,
    }
  },
  {
    path: '/crates',
    name: 'Crates',
    component: Crates,
    meta: {
      requiresAuth: true,
    }
  },
  {
    path: '/login',
    name: 'Login',
    component: Login,
    meta: {
      requiresAuth: false,
    }
  },
  {
    path: '/settings',
    name: 'Settings',
    component: Settings,
    meta: {
      requiresAuth: true,
    }
  },
  {
    path: '/publishdocs',
    name: 'PublishDocs',
    component: PublishDocs,
    meta: {
      requiresAuth: true,
    }
  },
  {
    path: '/crate',
    name: 'Crate',
    component: Crate,
    meta: {
      requiresAuth: true,
    }
  },
  {
    path: '/docqueue',
    name: 'DocQueue',
    component: DocQueue,
    meta: {
      requiresAuth: true,
    }
  },
  {
    path: '/me',
    name: 'Me',
    // Use Settings component - the beforeEach guard handles auth,
    // and if authenticated, redirects to /settings?tab=tokens
    component: Settings,
    meta: {
      requiresAuth: true,
    }
  }
]

const currentPath = window.location.pathname;
const base = currentPath.substring(0, currentPath.lastIndexOf("/") + 1);

const router = createRouter({
  history: createWebHistory(base),
  routes,
});

router.beforeEach(async (to) => {
  const store = useStore();

  // Handle /me route first - it always redirects somewhere
  if (to.path === '/me') {
    if (store.loggedIn) {
      // Logged in - go to settings tokens tab
      return { path: '/settings', query: { tab: 'tokens' } }
    } else {
      // Not logged in - go to login with redirect flag
      return { name: 'Login', query: { redirect: 'me' } }
    }
  }

  // Check if the "auth_required" setting is enabled in Kellnr.
  // If it is enabled, the user must be authenticated to view any page, except the login page.
  // If the user is not authenticated, he will be redirected to the login page.
  if (await auth_required()) {
    if (to.matched.some(record => record.meta.requiresAuth)) {
      if (!store.loggedIn) {
        const redirectFlag = to.path === '/settings' ? 'settings' : undefined
        return { name: 'Login', query: redirectFlag ? { redirect: redirectFlag } : {} }
      }
    }
  }
});

export default router
