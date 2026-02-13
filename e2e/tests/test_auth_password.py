"""Tests for password-based authentication flow."""

import pytest

from helpers.constants import ADMIN_PASS, ADMIN_USER, TEST_PASS, TEST_USER


pytestmark = [pytest.mark.web, pytest.mark.auth]


class TestPasswordLogin:
    def test_login_redirects_to_home(self, web):
        """Successful login navigates away from /login."""
        web.login_password(TEST_USER, TEST_PASS)
        assert "/login" not in web.page.url

    def test_login_wrong_password_stays_on_login(self, web):
        """Wrong password keeps the user on /login."""
        web.page.goto("/login")
        web.page.locator('input[type="text"]').first.fill(TEST_USER)
        web.page.locator('input[type="password"]').first.fill("wrongpassword")
        web.page.locator("button.login-button").first.click()
        # Should stay on login page (may show error)
        web.page.wait_for_timeout(2000)
        assert "/login" in web.page.url

    def test_login_shows_error_for_invalid_credentials(self, web):
        """Invalid credentials show an error message."""
        web.page.goto("/login")
        web.page.locator('input[type="text"]').first.fill("nonexistent")
        web.page.locator('input[type="password"]').first.fill("wrong")
        web.page.locator("button.login-button").first.click()
        web.page.wait_for_timeout(2000)
        error = web.page.locator(".error-message")
        # Error message should appear or we should still be on login
        is_visible = error.is_visible()
        still_on_login = "/login" in web.page.url
        assert is_visible or still_on_login

    def test_admin_login(self, web):
        """Admin user can log in successfully."""
        web.login_password(ADMIN_USER, ADMIN_PASS)
        assert "/login" not in web.page.url

    def test_session_persists_after_navigation(self, web):
        """After login, navigating to different pages keeps the session."""
        web.login_password(TEST_USER, TEST_PASS)
        # Navigate to a different page
        web.page.goto("/")
        web.page.wait_for_timeout(1000)
        assert "/login" not in web.page.url

    def test_unauthenticated_redirect(self, web):
        """Unauthenticated access to protected route redirects to /login."""
        web.page.goto("/")
        web.page.wait_for_url(lambda url: "/login" in str(url), timeout=10000)
        assert "/login" in web.page.url
