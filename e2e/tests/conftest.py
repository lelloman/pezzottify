"""Test-level fixtures for web and Android clients."""

import subprocess
from pathlib import Path

import pytest
from playwright.sync_api import sync_playwright

from helpers.config import E2EConfig
from helpers.web_client import WebClient

SCREENSHOT_DIR = Path("/test-results/screenshots")
LOGCAT_DIR = Path("/test-results/logcat")


@pytest.hookimpl(tryfirst=True, hookwrapper=True)
def pytest_runtest_makereport(item, call):
    """Stash test result on the item so fixtures can check for failure."""
    outcome = yield
    rep = outcome.get_result()
    setattr(item, f"rep_{rep.when}", rep)

    # Attach screenshot to HTML report if it exists
    if rep.when == "call" and rep.failed:
        name = item.name.replace("/", "_")
        screenshot = SCREENSHOT_DIR / f"{name}.png"
        if screenshot.exists():
            html_plugin = item.config.pluginmanager.getplugin("html")
            if html_plugin:
                extra = getattr(rep, "extra", [])
                extra.append(html_plugin.extras.image(str(screenshot)))
                rep.extra = extra


@pytest.fixture(scope="session")
def pw():
    """Session-scoped Playwright instance."""
    with sync_playwright() as p:
        yield p


@pytest.fixture(scope="session")
def browser(pw):
    """Session-scoped Playwright browser instance."""
    browser = pw.chromium.launch(headless=True)
    yield browser
    browser.close()


@pytest.fixture
def web(browser, config: E2EConfig, request) -> WebClient:
    """Single browser context for web tests."""
    client = WebClient(browser, config.web_url, name="web-1")
    with client:
        yield client
        # Capture screenshot on failure
        if hasattr(request.node, "rep_call") and request.node.rep_call.failed:
            try:
                SCREENSHOT_DIR.mkdir(parents=True, exist_ok=True)
                name = request.node.name.replace("/", "_")
                client.page.screenshot(
                    path=str(SCREENSHOT_DIR / f"{name}.png"), full_page=True
                )
            except Exception:
                pass


@pytest.fixture
def web2(browser, config: E2EConfig, request) -> WebClient:
    """Second browser context (different device)."""
    client = WebClient(browser, config.web_url, name="web-2")
    with client:
        yield client
        # Capture screenshot on failure
        if hasattr(request.node, "rep_call") and request.node.rep_call.failed:
            try:
                SCREENSHOT_DIR.mkdir(parents=True, exist_ok=True)
                name = request.node.name.replace("/", "_") + "_web2"
                client.page.screenshot(
                    path=str(SCREENSHOT_DIR / f"{name}.png"), full_page=True
                )
            except Exception:
                pass


@pytest.fixture
def web_clients(browser, config: E2EConfig):
    """Factory for creating N web clients."""
    clients: list[WebClient] = []

    def create(name: str | None = None) -> WebClient:
        n = len(clients) + 1
        client = WebClient(browser, config.web_url, name=name or f"web-{n}")
        client.start()
        clients.append(client)
        return client

    yield create

    for client in clients:
        client.close()


@pytest.fixture(autouse=True)
def clean_user_state(config: E2EConfig):
    """Clean up user state (liked content, playlists) after each test.

    This runs after every test to prevent state leakage between tests.
    Uses a dedicated API client so it doesn't interfere with test clients.
    Uses asyncio.run() so it works for both sync and async tests.
    """
    yield

    from helpers.api_client import CatalogApiClient
    from helpers.async_runner import run_async

    async def _cleanup():
        cleanup_api = CatalogApiClient(config.server_url)
        try:
            await cleanup_api.login(
                config.test_user, config.test_pass, device_uuid="cleanup-fixture"
            )
            # Unlike all liked content
            for content_type in ("track", "album", "artist"):
                try:
                    liked = await cleanup_api.get_liked_content(content_type)
                    for item_id in liked:
                        try:
                            await cleanup_api.unlike_content(content_type, item_id)
                        except Exception:
                            pass
                except Exception:
                    pass
            # Delete all playlists
            try:
                playlists = await cleanup_api.get_playlists()
                for playlist in playlists:
                    pid = playlist.get("id") if isinstance(playlist, dict) else playlist
                    if pid:
                        try:
                            await cleanup_api.delete_playlist(str(pid))
                        except Exception:
                            pass
            except Exception:
                pass
        except Exception:
            pass
        finally:
            await cleanup_api.close()

    run_async(_cleanup())


@pytest.fixture
def android(config: E2EConfig, request):
    """Android client - skips if no ANDROID_HOSTS configured."""
    if not config.android_hosts:
        pytest.skip("No ANDROID_HOSTS configured")

    from helpers.android_client import AndroidClient

    client = AndroidClient(config.android_hosts[0])
    yield client

    # Capture logcat on failure
    if hasattr(request.node, "rep_call") and request.node.rep_call.failed:
        try:
            LOGCAT_DIR.mkdir(parents=True, exist_ok=True)
            name = request.node.name.replace("/", "_")
            with open(LOGCAT_DIR / f"{name}.txt", "w") as f:
                subprocess.run(
                    ["adb", "-s", client._host, "logcat", "-d"],
                    stdout=f,
                    timeout=10,
                )
        except Exception:
            pass
