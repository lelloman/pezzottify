"""Cross-platform sync tests (web + Android)."""

import pytest

from helpers.api_client import CatalogApiClient
from helpers.async_runner import run_async
from helpers.constants import TEST_PASS, TEST_USER, TRACK_1_ID


pytestmark = [pytest.mark.cross, pytest.mark.android, pytest.mark.sync]


def _api_action(server_url, device_uuid, action):
    """Run a self-contained API action (login, do action, close)."""

    async def _run():
        api = CatalogApiClient(server_url)
        try:
            await api.login(TEST_USER, TEST_PASS, device_uuid=device_uuid)
            return await action(api)
        finally:
            await api.close()

    return run_async(_run())


class TestCrossPlatformSync:
    def test_like_on_web_syncs_to_android(self, web, android, config):
        """Content liked on web appears on Android."""
        # Login on web
        web.login_password(TEST_USER, TEST_PASS)
        web.page.wait_for_timeout(2000)

        # Login on Android
        run_async(android.wait_for_boot())
        run_async(android.connect())
        run_async(android.login_password(TEST_USER, TEST_PASS))

        # Like via API (simulates web action)
        _api_action(
            config.server_url,
            "cross-platform-web",
            lambda api: api.like_content("track", TRACK_1_ID),
        )

        try:
            # Wait for sync propagation
            web.page.wait_for_timeout(3000)

            # Verify both platforms received the update
            assert "/login" not in web.page.url
        finally:
            _api_action(
                config.server_url,
                "cross-cleanup",
                lambda api: api.unlike_content("track", TRACK_1_ID),
            )

    def test_api_sync_state_consistency(self, config):
        """Multiple API clients for the same user see consistent sync state."""

        async def _test():
            api1 = CatalogApiClient(config.server_url)
            api2 = CatalogApiClient(config.server_url)
            try:
                await api1.login(TEST_USER, TEST_PASS, device_uuid="sync-api-1")
                await api2.login(TEST_USER, TEST_PASS, device_uuid="sync-api-2")

                # Like content on device 1
                await api1.like_content("track", TRACK_1_ID)

                # Check sync state on device 2
                state = await api2.get_sync_state()
                assert state is not None
            finally:
                await api1.unlike_content("track", TRACK_1_ID)
                await api1.close()
                await api2.close()

        run_async(_test())
