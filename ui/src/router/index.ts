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
  }
]

const router = createRouter({
  history: createWebHistory(),
  routes
})

router.beforeEach(async (to) => {
  const store = useStore();

  // Check if the "auth_required" setting is enabled in Kellnr.
  // If it is enabled, the user must be authenticated to view any page, exept the login page.
  // If the user is not authenticated, he will be redirected to the login page.
  if (await auth_required()) {
    if (to.matched.some(record => record.meta.requiresAuth)) {
      if (!store.loggedIn) {
        console.log("Auth required. Redirecting to login page.");
        return { name: 'Login' }
      }
      else {
        console.log("Auth required. User is authenticated.");
      }
    }
  }
  else {
    console.log("Auth not required.");
  }
});

export default router
