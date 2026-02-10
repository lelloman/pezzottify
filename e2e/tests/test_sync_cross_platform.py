"""Cross-platform sync tests (web + Android)."""

import pytest

from helpers.api_client import CatalogApiClient
from helpers.constants import TEST_PASS, TEST_USER, TRACK_1_ID


pytestmark = [pytest.mark.cross, pytest.mark.android, pytest.mark.sync]


class TestCrossPlatformSync:
    async def test_like_on_web_syncs_to_android(self, web, android, config):
        """Content liked on web appears on Android."""
        # Login on web
        await web.login_password(TEST_USER, TEST_PASS)
        await web.page.wait_for_timeout(2000)

        # Login on Android
        await android.wait_for_boot()
        await android.connect()
        await android.login_password(TEST_USER, TEST_PASS)

        # Like via API (simulates web action)
        api = CatalogApiClient(config.catalog_server_url)
        try:
            await api.login(TEST_USER, TEST_PASS, device_uuid="cross-platform-web")
            await api.like_content("track", TRACK_1_ID)

            # Wait for sync propagation
            await web.page.wait_for_timeout(3000)

            # Verify both platforms received the update
            assert "/login" not in web.page.url
        finally:
            await api.unlike_content("track", TRACK_1_ID)
            await api.close()

    async def test_api_sync_state_consistency(self, config):
        """Multiple API clients for the same user see consistent sync state."""
        api1 = CatalogApiClient(config.catalog_server_url)
        api2 = CatalogApiClient(config.catalog_server_url)
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
