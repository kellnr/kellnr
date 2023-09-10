import { createRouter, createWebHashHistory } from 'vue-router';
import Crates from "../views/Crates.vue";
import Login from "../views/Login.vue";
import AdminSettings from "../views/AdminSettings.vue";
import UserSettings from "../views/UserSettings.vue";
import Eula from "../views/Eula.vue";
import PublishDocs from "../views/PublishDocs.vue";
import Crate from "../views/Crate.vue";
const routes = [
    {
        path: '/',
        name: 'Crates',
        component: Crates
    },
    {
        path: '/login',
        name: 'Login',
        component: Login
    },
    {
        path: '/adminsettings',
        name: 'AdminSettings',
        component: AdminSettings
    },
    {
        path: '/usersettings',
        name: 'UserSettings',
        component: UserSettings
    },
    {
        path: '/eula',
        name: 'Eula',
        component: Eula
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
    }
];
const router = createRouter({
    history: createWebHashHistory(),
    routes
});
export default router;
//# sourceMappingURL=index.js.map