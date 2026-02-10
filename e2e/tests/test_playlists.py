"""Tests for playlist CRUD operations."""

import pytest

from helpers.constants import (
    TEST_PASS,
    TEST_USER,
    TRACK_1_ID,
    TRACK_2_ID,
)


pytestmark = pytest.mark.web


class TestPlaylistApi:
    """Playlist operations via API."""

    async def test_create_and_list_playlist(self, user_api):
        result = await user_api.create_playlist("API Test Playlist", [TRACK_1_ID])
        playlist_id = result if isinstance(result, str) else result.get("id")
        try:
            playlists = await user_api.get_playlists()
            names = [p.get("name") for p in playlists]
            assert "API Test Playlist" in names
        finally:
            await user_api.delete_playlist(playlist_id)

    async def test_delete_playlist(self, user_api):
        result = await user_api.create_playlist("To Delete")
        playlist_id = result if isinstance(result, str) else result.get("id")
        await user_api.delete_playlist(playlist_id)
        playlists = await user_api.get_playlists()
        ids = [p.get("id") for p in playlists]
        assert playlist_id not in ids


class TestPlaylistWeb:
    """Playlist visibility in the web UI."""

    async def test_playlist_visible_in_ui(self, web, config):
        from helpers.api_client import CatalogApiClient

        api = CatalogApiClient(config.catalog_server_url)
        playlist_id = None
        try:
            await api.login(TEST_USER, TEST_PASS, device_uuid="playlist-test")
            result = await api.create_playlist(
                "UI Visible Playlist", [TRACK_1_ID, TRACK_2_ID]
            )
            playlist_id = result if isinstance(result, str) else result.get("id")

            await web.login_password(TEST_USER, TEST_PASS)
            await web.page.wait_for_timeout(3000)

            content = await web.page.content()
            assert "UI Visible Playlist" in content
        finally:
            if playlist_id:
                await api.delete_playlist(playlist_id)
            await api.close()
