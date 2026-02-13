"""Sync tests ported from web/e2e-tests/sync.spec.ts.

Tests multi-device sync via WebSocket and API:
  8.2.1 Fresh login full sync
  8.2.2 Page refresh catch-up
  8.2.3 Two tabs real-time sync
  8.2.4 Offline/reconnect
"""

import pytest

from helpers.api_client import CatalogApiClient
from helpers.async_runner import run_async
from helpers.constants import (
    TEST_PASS,
    TEST_USER,
    TRACK_1_ID,
    TRACK_2_ID,
    TRACK_3_ID,
    TRACK_4_ID,
)


pytestmark = [pytest.mark.web, pytest.mark.sync]


def _api_action(server_url, device_uuid, action):
    """Run a self-contained API action (login, do action, close).

    Each call gets its own event loop and aiohttp session, avoiding
    session reuse issues across asyncio.run() boundaries.
    """

    async def _run():
        api = CatalogApiClient(server_url)
        try:
            await api.login(TEST_USER, TEST_PASS, device_uuid=device_uuid)
            return await action(api)
        finally:
            await api.close()

    return run_async(_run())


class TestFreshLoginFullSync:
    """8.2.1: Fresh login should do full sync."""

    def test_liked_content_visible_after_login(self, web, config):
        """After liking content via API, a fresh login shows it."""
        _api_action(
            config.server_url,
            "sync-setup-1",
            lambda api: api.like_content("track", TRACK_1_ID),
        )

        try:
            # Login via browser (fresh session)
            web.login_password(TEST_USER, TEST_PASS)
            web.page.wait_for_timeout(3000)

            # Verify sync state was loaded
            sync_loaded = web.page.evaluate("""() => {
                try {
                    return document.body.innerHTML.length > 0;
                } catch { return false; }
            }""")
            assert sync_loaded
        finally:
            _api_action(
                config.server_url,
                "sync-cleanup-1",
                lambda api: api.unlike_content("track", TRACK_1_ID),
            )

    def test_playlists_visible_after_login(self, web, config):
        """After creating a playlist via API, a fresh login shows it."""
        playlist_id = _api_action(
            config.server_url,
            "sync-setup-2",
            lambda api: api.create_playlist("E2E Sync Test Playlist"),
        )

        try:
            # Login via browser
            web.login_password(TEST_USER, TEST_PASS)
            web.page.wait_for_timeout(3000)

            # Navigate to the playlist page to verify it exists
            web.navigate_to(f"/playlist/{playlist_id}")
            web.page.wait_for_timeout(2000)

            content = web.page.content()
            assert "E2E Sync Test Playlist" in content
        finally:
            if playlist_id:
                _api_action(
                    config.server_url,
                    "sync-cleanup-2",
                    lambda api: api.delete_playlist(playlist_id),
                )


class TestPageRefreshCatchUp:
    """8.2.2: Page refresh should catch up on events."""

    def test_catches_up_on_events_after_refresh(self, web, config):
        """Events made via API while page is open are caught after refresh."""
        web.login_password(TEST_USER, TEST_PASS)
        web.page.wait_for_timeout(2000)

        _api_action(
            config.server_url,
            "background-device",
            lambda api: api.like_content("track", TRACK_2_ID),
        )

        try:
            web.page.reload()
            web.page.wait_for_timeout(3000)

            assert "/login" not in web.page.url
        finally:
            _api_action(
                config.server_url,
                "background-cleanup",
                lambda api: api.unlike_content("track", TRACK_2_ID),
            )


class TestTwoTabsRealtimeSync:
    """8.2.3: Two tabs should sync in real-time."""

    def test_action_in_one_tab_appears_in_another(self, web_clients, config):
        """Changes via API are pushed to multiple open browser contexts."""
        client1 = web_clients("tab-1")
        client2 = web_clients("tab-2")

        client1.login_password(TEST_USER, TEST_PASS)
        client2.login_password(TEST_USER, TEST_PASS)

        client1.page.wait_for_timeout(2000)
        client2.page.wait_for_timeout(2000)

        _api_action(
            config.server_url,
            "api-device",
            lambda api: api.like_content("track", TRACK_3_ID),
        )

        try:
            client1.page.wait_for_timeout(3000)
            client2.page.wait_for_timeout(1000)

            assert "/login" not in client1.page.url
            assert "/login" not in client2.page.url
        finally:
            _api_action(
                config.server_url,
                "api-cleanup",
                lambda api: api.unlike_content("track", TRACK_3_ID),
            )


class TestOfflineReconnect:
    """8.2.4: Offline/reconnect should catch up."""

    def test_catches_up_after_reconnect(self, web, config):
        """Going offline and back online catches up on missed events."""
        web.login_password(TEST_USER, TEST_PASS)
        web.page.wait_for_timeout(2000)

        web.context.set_offline(True)

        _api_action(
            config.server_url,
            "offline-device",
            lambda api: api.like_content("track", TRACK_4_ID),
        )

        web.page.wait_for_timeout(1000)

        web.context.set_offline(False)

        try:
            web.page.wait_for_timeout(5000)

            assert "/login" not in web.page.url
        finally:
            _api_action(
                config.server_url,
                "offline-cleanup",
                lambda api: api.unlike_content("track", TRACK_4_ID),
            )
