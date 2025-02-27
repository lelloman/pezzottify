import { createRouter, createWebHistory } from 'vue-router'
import HomeView from '../views/HomeView.vue'
import LoginView from '../views/LoginView.vue'
import { useAuthStore } from '../store/auth.js';
import axios from 'axios';

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      name: 'home',
      component: HomeView,
      meta: { requiresAuth: true },
      children: [
        {
          path: '/search/:query?',
          name: 'search_results',
          component: HomeView,
          meta: { requiresAuth: true }
        },
        {
          path: '/track/:trackId?',
          name: 'track',
          component: HomeView,
          meta: { requiresAuth: true }
        },
        {
          path: '/album/:albumId?',
          name: 'album',
          component: HomeView,
          meta: { requiresAuth: true }
        },
        {
          path: '/artist/:artistId?',
          name: 'artist',
          component: HomeView,
          meta: { requiresAuth: true }
        },
        {
          path: '/playlist/:playlistId?',
          name: 'playlist',
          component: HomeView,
          meta: { requiresAuth: true }
        }
      ]
    },

    {
      path: '/login',
      name: 'login',
      component: LoginView,
    },
    {
      path: '/logout',
      name: 'logout',
      beforeEnter: async (to, from, next) => {
        try {
          await axios.get('/v1/auth/logout');
          useAuthStore().logout();
          next('/login');
        } catch (error) {
          console.error('Logout failed', error);
          if (error.response.status == 403) {
            useAuthStore().logout();
            next('/login');
          } else {
            next('/');
          }
        }
      },
    }
  ],
})

router.beforeEach((to, from, next) => {
  const authStore = useAuthStore();

  console.log("beforeEach to: " + to + " from: " + from + " is authenticaed: " + authStore.isAuthenticated);
  if (to.meta.requiresAuth && !authStore.isAuthenticated) {
    next('/login');
  } else {
    next();
  }
});

export default router
