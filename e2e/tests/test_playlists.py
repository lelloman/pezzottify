"""Tests for playlist CRUD operations."""

import pytest

from helpers.api_client import CatalogApiClient
from helpers.async_runner import run_async
from helpers.constants import (
    TEST_PASS,
    TEST_USER,
    TRACK_1_ID,
    TRACK_2_ID,
)


pytestmark = pytest.mark.web


class TestPlaylistApi:
    """Playlist operations via API."""

    def test_create_and_list_playlist(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid="playlist-create")
                playlist_id = await api.create_playlist("API Test Playlist", [TRACK_1_ID])
                try:
                    playlist_ids = await api.get_playlists()
                    assert playlist_id in playlist_ids

                    # Verify the playlist details
                    details = await api.get_playlist(playlist_id)
                    assert details["name"] == "API Test Playlist"
                finally:
                    await api.delete_playlist(playlist_id)
            finally:
                await api.close()

        run_async(_test())

    def test_delete_playlist(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid="playlist-delete")
                playlist_id = await api.create_playlist("To Delete")
                await api.delete_playlist(playlist_id)
                playlist_ids = await api.get_playlists()
                assert playlist_id not in playlist_ids
            finally:
                await api.close()

        run_async(_test())


class TestPlaylistWeb:
    """Playlist visibility in the web UI."""

    def test_playlist_visible_in_ui(self, web, config):
        async def _setup():
            api = CatalogApiClient(config.server_url)
            await api.login(TEST_USER, TEST_PASS, device_uuid="playlist-test")
            playlist_id = await api.create_playlist(
                "UI Visible Playlist", [TRACK_1_ID, TRACK_2_ID]
            )
            await api.close()
            return playlist_id

        async def _cleanup(playlist_id):
            api = CatalogApiClient(config.server_url)
            await api.login(TEST_USER, TEST_PASS, device_uuid="playlist-cleanup")
            await api.delete_playlist(playlist_id)
            await api.close()

        playlist_id = run_async(_setup())

        try:
            web.login_password(TEST_USER, TEST_PASS)
            web.page.wait_for_timeout(3000)

            # Navigate to the playlist page
            web.navigate_to(f"/playlist/{playlist_id}")
            web.page.wait_for_timeout(2000)

            content = web.page.content()
            assert "UI Visible Playlist" in content
        finally:
            run_async(_cleanup(playlist_id))
