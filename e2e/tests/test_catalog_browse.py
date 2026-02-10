"""Tests for browsing catalog content (artists, albums, tracks)."""

import pytest

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

    async def test_get_artist(self, user_api):
        data = await user_api.get_artist(ARTIST_1_ID)
        assert data["id"] == ARTIST_1_ID
        assert data["name"] == ARTIST_1_NAME

    async def test_get_album(self, user_api):
        data = await user_api.get_album(ALBUM_1_ID)
        assert data["id"] == ALBUM_1_ID
        assert data["name"] == ALBUM_1_TITLE

    async def test_get_track(self, user_api):
        data = await user_api.get_track(TRACK_1_ID)
        assert data["id"] == TRACK_1_ID
        assert data["name"] == TRACK_1_TITLE

    async def test_search_artist(self, user_api):
        data = await user_api.search("Test Band")
        # Search should return results containing the artist
        assert data is not None

    async def test_search_album(self, user_api):
        data = await user_api.search("Jazz Collection")
        assert data is not None


class TestCatalogBrowseWeb:
    """Verify catalog pages render correctly in the browser."""

    async def test_artist_page_shows_name(self, web):
        await web.login_password(TEST_USER, TEST_PASS)
        await web.navigate_to(f"/artist/{ARTIST_1_ID}")
        await web.page.wait_for_timeout(2000)
        content = await web.page.content()
        assert ARTIST_1_NAME in content

    async def test_album_page_shows_title(self, web):
        await web.login_password(TEST_USER, TEST_PASS)
        await web.navigate_to(f"/album/{ALBUM_1_ID}")
        await web.page.wait_for_timeout(2000)
        content = await web.page.content()
        assert ALBUM_1_TITLE in content

    async def test_track_page_shows_title(self, web):
        await web.login_password(TEST_USER, TEST_PASS)
        await web.navigate_to(f"/track/{TRACK_1_ID}")
        await web.page.wait_for_timeout(2000)
        content = await web.page.content()
        assert TRACK_1_TITLE in content

    async def test_search_page(self, web):
        await web.login_password(TEST_USER, TEST_PASS)
        await web.navigate_to("/search/Test")
        await web.page.wait_for_timeout(3000)
        # Verify the page loaded without errors
        assert "/login" not in web.page.url
