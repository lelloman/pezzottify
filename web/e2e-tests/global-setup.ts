/**
 * Global setup for Playwright E2E tests.
 *
 * This runs once before all tests to verify the test environment is ready.
 */

const SERVER_URL = process.env.E2E_SERVER_URL || 'http://localhost:3099';
const WEB_URL = process.env.E2E_WEB_URL || 'http://localhost:5199';

async function globalSetup() {
  console.log('Checking test environment...');

  // Check if the catalog-server is running
  try {
    const healthResponse = await fetch(`${SERVER_URL}/v1/health`);
    if (!healthResponse.ok) {
      throw new Error(`Server health check failed: ${healthResponse.status}`);
    }
    console.log(`  Catalog server: OK (${SERVER_URL})`);
  } catch (error) {
    console.error(`\n  ERROR: Catalog server not running at ${SERVER_URL}`);
    console.error('  Please start the server with test fixtures before running E2E tests.');
    console.error('\n  Example:');
    console.error('    cd catalog-server');
    console.error('    cargo run --features fast -- test.db user.db --media-path=../test-media --port=3099\n');
    throw new Error('Catalog server not available');
  }

  // Check if the web server is running
  try {
    const webResponse = await fetch(WEB_URL);
    if (!webResponse.ok) {
      throw new Error(`Web server check failed: ${webResponse.status}`);
    }
    console.log(`  Web server: OK (${WEB_URL})`);
  } catch (error) {
    console.error(`\n  ERROR: Web server not running at ${WEB_URL}`);
    console.error('  Please start the web dev server before running E2E tests.');
    console.error('\n  Example:');
    console.error('    cd web');
    console.error('    npm run dev -- --port 5199\n');
    throw new Error('Web server not available');
  }

  console.log('Test environment ready!\n');
}

export default globalSetup;
