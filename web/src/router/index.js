import { createRouter, createWebHistory } from "vue-router";
import HomeView from "../views/HomeView.vue";
import LoginView from "../views/LoginView.vue";
import AdminView from "../views/AdminView.vue";
import { useAuthStore } from "../store/auth.js";
import axios from "axios";

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: "/",
      name: "home",
      component: HomeView,
      meta: { requiresAuth: true },
      children: [
        {
          path: "/search/:query?",
          name: "search_results",
          component: HomeView,
          meta: { requiresAuth: true },
        },
        {
          path: "/track/:trackId?",
          name: "track",
          component: HomeView,
          meta: { requiresAuth: true },
        },
        {
          path: "/album/:albumId?",
          name: "album",
          component: HomeView,
          meta: { requiresAuth: true },
        },
        {
          path: "/artist/:artistId?",
          name: "artist",
          component: HomeView,
          meta: { requiresAuth: true },
        },
        {
          path: "/playlist/:playlistId?",
          name: "playlist",
          component: HomeView,
          meta: { requiresAuth: true },
        },
        {
          path: "/settings",
          name: "settings",
          component: HomeView,
          meta: { requiresAuth: true },
        },
        {
          path: "/requests",
          name: "requests",
          component: HomeView,
          meta: { requiresAuth: true },
        },
      ],
    },

    {
      path: "/admin",
      component: AdminView,
      meta: { requiresAuth: true },
      children: [
        {
          path: "",
          name: "admin",
          redirect: "/admin/users",
        },
        {
          path: "users",
          name: "admin-users",
          meta: { requiresAuth: true, section: "users" },
        },
        {
          path: "analytics",
          name: "admin-analytics",
          meta: { requiresAuth: true, section: "analytics" },
        },
        {
          path: "server",
          name: "admin-server",
          meta: { requiresAuth: true, section: "server" },
        },
        {
          path: "downloads",
          name: "admin-downloads",
          meta: { requiresAuth: true, section: "downloads" },
        },
        {
          path: "batches",
          name: "admin-batches",
          meta: { requiresAuth: true, section: "batches" },
        },
      ],
    },

    {
      path: "/login",
      name: "login",
      component: LoginView,
    },
    {
      path: "/logout",
      name: "logout",
      beforeEnter: async (to, from, next) => {
        try {
          await axios.get("/v1/auth/logout");
          useAuthStore().logout();
          next("/login");
        } catch (error) {
          console.error("Logout failed", error);
          if (error.response.status == 403) {
            useAuthStore().logout();
            next("/login");
          } else {
            next("/");
          }
        }
      },
    },
  ],
});

router.beforeEach(async (to, from, next) => {
  const authStore = useAuthStore();

  // Wait for initial session check if not done yet
  if (!authStore.sessionChecked) {
    await authStore.checkSession();
  }

  if (to.meta.requiresAuth && !authStore.isAuthenticated) {
    next("/login");
  } else if (to.path === "/login" && authStore.isAuthenticated) {
    // Already authenticated, redirect to home
    next("/");
  } else {
    next();
  }
});

export default router;
