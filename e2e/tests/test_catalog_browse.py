"""Tests for browsing catalog content (artists, albums, tracks)."""

import pytest

from helpers.api_client import CatalogApiClient
from helpers.async_runner import run_async
from helpers.constants import (
    ALBUM_1_ID,
    ALBUM_1_TITLE,
    ARTIST_1_ID,
    ARTIST_1_NAME,
    TEST_PASS,
    TEST_USER,
    TRACK_1_ID,
    TRACK_1_TITLE,
)


pytestmark = pytest.mark.web


class TestCatalogBrowseApi:
    """Verify catalog content is accessible via the API."""

    def test_get_artist(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid="browse-artist")
                data = await api.get_artist(ARTIST_1_ID)
                assert data["id"] == ARTIST_1_ID
                assert data["name"] == ARTIST_1_NAME
            finally:
                await api.close()

        run_async(_test())

    def test_get_album(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid="browse-album")
                data = await api.get_album(ALBUM_1_ID)
                assert data["id"] == ALBUM_1_ID
                assert data["name"] == ALBUM_1_TITLE
            finally:
                await api.close()

        run_async(_test())

    def test_get_track(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid="browse-track")
                data = await api.get_track(TRACK_1_ID)
                assert data["id"] == TRACK_1_ID
                assert data["name"] == TRACK_1_TITLE
            finally:
                await api.close()

        run_async(_test())

    def test_search_artist(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid="search-artist")
                data = await api.search("Test Band")
                assert data is not None
            finally:
                await api.close()

        run_async(_test())

    def test_search_album(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid="search-album")
                data = await api.search("Jazz Collection")
                assert data is not None
            finally:
                await api.close()

        run_async(_test())


class TestCatalogBrowseWeb:
    """Verify catalog pages render correctly in the browser."""

    def test_artist_page_shows_name(self, web):
        web.login_password(TEST_USER, TEST_PASS)
        web.navigate_to(f"/artist/{ARTIST_1_ID}")
        web.page.wait_for_timeout(2000)
        content = web.page.content()
        assert ARTIST_1_NAME in content

    def test_album_page_shows_title(self, web):
        web.login_password(TEST_USER, TEST_PASS)
        web.navigate_to(f"/album/{ALBUM_1_ID}")
        web.page.wait_for_timeout(2000)
        content = web.page.content()
        assert ALBUM_1_TITLE in content

    def test_track_page_shows_title(self, web):
        web.login_password(TEST_USER, TEST_PASS)
        web.navigate_to(f"/track/{TRACK_1_ID}")
        web.page.wait_for_timeout(2000)
        content = web.page.content()
        assert TRACK_1_TITLE in content

    def test_search_page(self, web):
        web.login_password(TEST_USER, TEST_PASS)
        web.navigate_to("/search/Test")
        web.page.wait_for_timeout(3000)
        # Verify the page loaded without errors
        assert "/login" not in web.page.url
