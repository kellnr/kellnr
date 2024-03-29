import {createRouter, createWebHistory} from 'vue-router'
import Crates from "../views/Crates.vue";
import Login from "../views/Login.vue";
import Settings from "../views/Settings.vue";
import PublishDocs from "../views/PublishDocs.vue";
import Crate from "../views/Crate.vue";
import DocQueue from "../views/DocQueue.vue";
import Landing from "../views/Landing.vue";

const routes = [
  {
    path: '/',
    name: 'Landing',
    component: Landing
  },
  {
    path: '/crates',
    name: 'Crates',
    component: Crates
  },
  {
    path: '/login',
    name: 'Login',
    component: Login,
  },
  {
    path: '/settings',
    name: 'Settings',
    component: Settings
  },
  {
    path: '/publishdocs',
    name: 'PublishDocs',
    component: PublishDocs
  },
  {
    path: '/crate',
    name: 'Crate',
    component: Crate,
  },
  {
    path: '/docqueue',
    name: 'DocQueue',
    component: DocQueue,
  }
]

const router = createRouter({
  history: createWebHistory(),
  routes
})

export default router
