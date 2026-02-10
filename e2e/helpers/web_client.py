"""Playwright browser wrapper for web E2E tests."""

from __future__ import annotations

from playwright.async_api import Browser, BrowserContext, Page


class WebClient:
    """Wraps a Playwright BrowserContext as a single 'device'.

    Each WebClient has its own isolated browser context (cookies, storage).
    """

    def __init__(self, browser: Browser, base_url: str, name: str = "default"):
        self._browser = browser
        self._base_url = base_url.rstrip("/")
        self._name = name
        self.context: BrowserContext | None = None
        self.page: Page | None = None

    async def start(self) -> "WebClient":
        self.context = await self._browser.new_context(
            base_url=self._base_url,
            ignore_https_errors=True,
        )
        self.page = await self.context.new_page()
        return self

    async def login_password(self, username: str, password: str) -> None:
        """Login via the password form on /login."""
        await self.page.goto("/login")
        await self.page.locator('input[type="text"]').first.fill(username)
        await self.page.locator('input[type="password"]').first.fill(password)
        await self.page.locator("button.login-button").first.click()
        # Wait for navigation away from login
        await self.page.wait_for_url(
            lambda url: "/login" not in str(url),
            timeout=15000,
        )

    async def login_oidc(self) -> None:
        """Login via the OIDC button, filling the LelloAuth form."""
        await self.page.goto("/login")
        await self.page.locator("button.oidc-button").first.click()
        # LelloAuth form - wait for redirect to OIDC provider
        await self.page.wait_for_url(lambda url: "mock-oidc" in str(url), timeout=10000)
        # Fill OIDC login form (LelloAuth test UI)
        await self.page.locator('input[name="username"], input[type="text"]').first.fill(
            "testuser"
        )
        await self.page.locator('input[name="password"], input[type="password"]').first.fill(
            "testpass123"
        )
        await self.page.locator('button[type="submit"]').first.click()
        # Wait for redirect back to app
        await self.page.wait_for_url(
            lambda url: "mock-oidc" not in str(url),
            timeout=15000,
        )

    async def navigate_to(self, path: str) -> None:
        await self.page.goto(path)

    async def close(self) -> None:
        if self.context:
            await self.context.close()
            self.context = None
            self.page = None

    async def __aenter__(self) -> "WebClient":
        return await self.start()

    async def __aexit__(self, *args) -> None:
        await self.close()
