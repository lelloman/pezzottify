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
      meta: { requiresAuth: true }
    },
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
      path: '/login',
      name: 'login',
      component: LoginView,
    },
    {
      path: '/logout',
      name: 'logout',
      beforeEnter: async (to, from, next) => {
        try {
          // Call your logout API
          await axios.get('/v1/auth/logout');  // Replace with your actual logout endpoint
          // Redirect to the home page (or any other page)
          useAuthStore().logout();
          next('/login');
        } catch (error) {
          console.error('Logout failed', error);
          // Optionally handle the error (e.g., redirect or show an error page)
          next('/');
        }
      },
    }
  ],
})

router.beforeEach((to, from, next) => {
  const authStore = useAuthStore();

  console.log("beforeEach to: " + to + " from: " + from + " is authenticaed: " + authStore.isAuthenticated);
  if (to.meta.requiresAuth && !authStore.isAuthenticated) {
    next('/login'); // Redirect to login if not authenticated
  } else {
    next();
  }
});

export default router
