import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for E2E sync tests.
 *
 * Before running tests, you need to:
 * 1. Start the catalog-server on port 3099 (or set E2E_SERVER_URL)
 * 2. Start the web dev server on port 5199 (or set E2E_WEB_URL)
 *
 * The servers should have a test user:
 * - Username: testuser
 * - Password: testpassword
 */
export default defineConfig({
  testDir: './e2e-tests',
  fullyParallel: false, // Run tests serially for sync tests
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1, // Single worker for sync tests
  reporter: 'list',
  timeout: 60000,
  globalSetup: './e2e-tests/global-setup.ts',

  use: {
    // Base URL for the web app
    baseURL: process.env.E2E_WEB_URL || 'http://localhost:5199',
    // Use headless mode by default
    headless: true,
    // Trace on first retry
    trace: 'on-first-retry',
    // Screenshot on failure
    screenshot: 'only-on-failure',
    // Default timeout for actions
    actionTimeout: 10000,
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
