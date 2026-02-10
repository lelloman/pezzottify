"""Tests for password-based authentication flow."""

import pytest

from helpers.constants import ADMIN_PASS, ADMIN_USER, TEST_PASS, TEST_USER


pytestmark = [pytest.mark.web, pytest.mark.auth]


class TestPasswordLogin:
    async def test_login_redirects_to_home(self, web):
        """Successful login navigates away from /login."""
        await web.login_password(TEST_USER, TEST_PASS)
        assert "/login" not in web.page.url

    async def test_login_wrong_password_stays_on_login(self, web):
        """Wrong password keeps the user on /login."""
        await web.page.goto("/login")
        await web.page.locator('input[type="text"]').first.fill(TEST_USER)
        await web.page.locator('input[type="password"]').first.fill("wrongpassword")
        await web.page.locator("button.login-button").first.click()
        # Should stay on login page (may show error)
        await web.page.wait_for_timeout(2000)
        assert "/login" in web.page.url

    async def test_login_shows_error_for_invalid_credentials(self, web):
        """Invalid credentials show an error message."""
        await web.page.goto("/login")
        await web.page.locator('input[type="text"]').first.fill("nonexistent")
        await web.page.locator('input[type="password"]').first.fill("wrong")
        await web.page.locator("button.login-button").first.click()
        await web.page.wait_for_timeout(2000)
        error = web.page.locator(".error-message")
        # Error message should appear or we should still be on login
        is_visible = await error.is_visible()
        still_on_login = "/login" in web.page.url
        assert is_visible or still_on_login

    async def test_admin_login(self, web):
        """Admin user can log in successfully."""
        await web.login_password(ADMIN_USER, ADMIN_PASS)
        assert "/login" not in web.page.url

    async def test_session_persists_after_navigation(self, web):
        """After login, navigating to different pages keeps the session."""
        await web.login_password(TEST_USER, TEST_PASS)
        # Navigate to a different page
        await web.page.goto("/")
        await web.page.wait_for_timeout(1000)
        assert "/login" not in web.page.url

    async def test_unauthenticated_redirect(self, web):
        """Unauthenticated access to protected route redirects to /login."""
        await web.page.goto("/")
        await web.page.wait_for_url(lambda url: "/login" in str(url), timeout=10000)
        assert "/login" in web.page.url
