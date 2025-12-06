/**
 * Playwright test fixtures for sync E2E tests.
 */

import { test as base, expect, Page, BrowserContext } from '@playwright/test';

// Test user credentials (must match server test fixtures)
export const TEST_USER = 'testuser';
export const TEST_PASS = 'testpassword';

const SERVER_URL = process.env.E2E_SERVER_URL || 'http://localhost:3099';

// Generate unique device UUIDs for each test
let deviceCounter = 0;
function generateDeviceUuid(): string {
  return `e2e-device-${Date.now()}-${++deviceCounter}`;
}

/**
 * Extended test fixture with authenticated page
 */
export const test = base.extend<{
  authenticatedPage: Page;
  secondAuthenticatedContext: BrowserContext;
}>({
  // Provides a page already logged in as the test user
  authenticatedPage: async ({ page }, use) => {
    await loginPage(page);
    await use(page);
  },

  // Provides a second browser context logged in as the same user (different device)
  secondAuthenticatedContext: async ({ browser }, use) => {
    const context = await browser.newContext();
    const page = await context.newPage();
    await loginPage(page, 'device-2');
    await use(context);
    await context.close();
  },
});

/**
 * Login to the app via the UI
 */
export async function loginPage(page: Page, deviceSuffix: string = 'device-1'): Promise<void> {
  // Navigate to login page
  await page.goto('/login');

  // Fill in credentials
  await page.locator('input[name="username"], input[type="text"]').first().fill(TEST_USER);
  await page.locator('input[name="password"], input[type="password"]').first().fill(TEST_PASS);

  // Submit login
  await page.locator('button[type="submit"], button:has-text("Login"), button:has-text("Sign in")').first().click();

  // Wait for navigation away from login page
  await page.waitForURL((url) => !url.pathname.includes('/login'), { timeout: 10000 });
}

/**
 * Login via API and return cookies for programmatic setup
 */
export async function loginApi(deviceUuid?: string): Promise<{
  cookies: { name: string; value: string }[];
  deviceId: number;
}> {
  const uuid = deviceUuid || generateDeviceUuid();

  const response = await fetch(`${SERVER_URL}/v1/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      user_handle: TEST_USER,
      password: TEST_PASS,
      device_uuid: uuid,
      device_type: 'web',
      device_name: `E2E Test ${uuid}`,
    }),
  });

  if (!response.ok) {
    throw new Error(`Login failed: ${response.status} ${await response.text()}`);
  }

  const data = await response.json();

  // Extract session cookie
  const setCookie = response.headers.get('set-cookie');
  const cookies: { name: string; value: string }[] = [];

  if (setCookie) {
    const match = setCookie.match(/session_token=([^;]+)/);
    if (match) {
      cookies.push({ name: 'session_token', value: match[1] });
    }
  }

  return { cookies, deviceId: data.device_id };
}

/**
 * Get the current sync state via API
 */
export async function getSyncState(sessionToken: string): Promise<{
  seq: number;
  likedContent: { tracks: string[]; albums: string[]; artists: string[] };
  settings: Record<string, unknown>;
  playlists: Array<{ id: string; name: string; track_ids: string[] }>;
}> {
  const response = await fetch(`${SERVER_URL}/v1/sync/state`, {
    headers: { Cookie: `session_token=${sessionToken}` },
  });

  if (!response.ok) {
    throw new Error(`Failed to get sync state: ${response.status}`);
  }

  return response.json();
}

/**
 * Get sync events since a given sequence
 */
export async function getSyncEvents(
  sessionToken: string,
  since: number
): Promise<{ seq: number; events: unknown[] }> {
  const response = await fetch(`${SERVER_URL}/v1/sync/events?since=${since}`, {
    headers: { Cookie: `session_token=${sessionToken}` },
  });

  if (!response.ok) {
    throw new Error(`Failed to get sync events: ${response.status}`);
  }

  return response.json();
}

/**
 * Like content via API
 */
export async function likeContent(
  sessionToken: string,
  contentType: 'track' | 'album' | 'artist',
  contentId: string
): Promise<void> {
  const response = await fetch(`${SERVER_URL}/v1/user/liked?type=${contentType}&id=${contentId}`, {
    method: 'PUT',
    headers: { Cookie: `session_token=${sessionToken}` },
  });

  if (!response.ok) {
    throw new Error(`Failed to like content: ${response.status}`);
  }
}

/**
 * Unlike content via API
 */
export async function unlikeContent(
  sessionToken: string,
  contentType: 'track' | 'album' | 'artist',
  contentId: string
): Promise<void> {
  const response = await fetch(`${SERVER_URL}/v1/user/liked?type=${contentType}&id=${contentId}`, {
    method: 'DELETE',
    headers: { Cookie: `session_token=${sessionToken}` },
  });

  if (!response.ok) {
    throw new Error(`Failed to unlike content: ${response.status}`);
  }
}

/**
 * Create a playlist via API
 */
export async function createPlaylist(
  sessionToken: string,
  name: string,
  trackIds: string[] = []
): Promise<string> {
  const response = await fetch(`${SERVER_URL}/v1/user/playlists`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Cookie: `session_token=${sessionToken}`,
    },
    body: JSON.stringify({ name, track_ids: trackIds }),
  });

  if (!response.ok) {
    throw new Error(`Failed to create playlist: ${response.status}`);
  }

  return response.json();
}

/**
 * Delete a playlist via API
 */
export async function deletePlaylist(sessionToken: string, playlistId: string): Promise<void> {
  const response = await fetch(`${SERVER_URL}/v1/user/playlists/${playlistId}`, {
    method: 'DELETE',
    headers: { Cookie: `session_token=${sessionToken}` },
  });

  if (!response.ok) {
    throw new Error(`Failed to delete playlist: ${response.status}`);
  }
}

export { expect };
