import { createRouter, createWebHistory } from 'vue-router'
import { auth_required } from "../common/auth";
import { useStore } from "../store/store";
import { settingsService } from "../services";
import { isSuccess } from "../services/api";
import { startSsoLogin } from "../common/ssoLogin";
import { setPageTitle } from "../common/pageTitle";

const Crates = () => import("../views/Crates.vue")
const Login = () => import("../views/Login.vue")
const Settings = () => import("../views/Settings.vue")
const PublishDocs = () => import("../views/PublishDocs.vue")
const Crate = () => import("../views/Crate.vue")
const DocQueue = () => import("../views/DocQueue.vue")
const Landing = () => import("../views/Landing.vue")

const routes = [
  {
    path: '/',
    name: 'Landing',
    component: Landing,
    meta: {
      title: 'Home',
      requiresAuth: true,
    }
  },
  {
    path: '/crates',
    name: 'Crates',
    component: Crates,
    meta: {
      title: 'Search',
      requiresAuth: true,
    }
  },
  {
    path: '/login',
    name: 'Login',
    component: Login,
    meta: {
      title: 'Log in',
      requiresAuth: false,
    }
  },
  {
    path: '/settings',
    name: 'Settings',
    component: Settings,
    meta: {
      title: 'Settings',
      requiresAuth: true,
    }
  },
  {
    path: '/publishdocs',
    name: 'PublishDocs',
    component: PublishDocs,
    meta: {
      title: 'Publish docs',
      requiresAuth: true,
    }
  },
  {
    path: '/crate',
    name: 'Crate',
    component: Crate,
    meta: {
      // Placeholder until the crate page sets "<name> <version>"
      title: 'Crate',
      requiresAuth: true,
    }
  },
  {
    path: '/docqueue',
    name: 'DocQueue',
    component: DocQueue,
    meta: {
      title: 'Doc queue',
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

  // With auto_redirect enabled, skip the login page and go straight to the provider.
  if (to.name === 'Login' && to.query['from'] !== 'logout') {
    const cfg = await settingsService.getOAuth2Config()
    if (isSuccess(cfg) && cfg.data.enabled && cfg.data.available && cfg.data.auto_redirect) {
      startSsoLogin()
      return false
    }
  }

  // Check if the "auth_required" setting is enabled in Kellnr.
  // If it is enabled, the user must be authenticated to view any page, except the login page.
  // If the user is not authenticated, he will be redirected to the login page.
  if (await auth_required()) {
    if (to.matched.some(record => record.meta.requiresAuth)) {
      if (!store.loggedIn) {
        const redirectQuery = to.path === '/settings' ? { redirect: 'settings' } : {};
        const fromQuery = to.query['from'] ? { from: to.query['from'] } : {};
        return { name: 'Login', query: { ...redirectQuery, ...fromQuery } }
      }
    }
  }
});

// Pages with a dynamic title (the crate page) set it themselves once their data is loaded.
router.afterEach((to) => {
  const title = to.meta['title'];
  if (typeof title === 'string') {
    setPageTitle(title);
  }
});

export default router
