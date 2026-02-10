"""ADB + uiautomator2 wrapper for Android E2E tests."""

from __future__ import annotations

import asyncio
import subprocess


class AndroidClient:
    """Wraps ADB + uiautomator2 for Android device interaction."""

    def __init__(self, host: str):
        self._host = host
        self.device = None

    async def wait_for_boot(self, timeout: int = 120) -> None:
        """Wait for emulator to finish booting."""
        deadline = asyncio.get_event_loop().time() + timeout
        while asyncio.get_event_loop().time() < deadline:
            try:
                result = subprocess.run(
                    ["adb", "-s", self._host, "shell", "getprop", "sys.boot_completed"],
                    capture_output=True,
                    text=True,
                    timeout=5,
                )
                if result.stdout.strip() == "1":
                    return
            except (subprocess.TimeoutExpired, subprocess.CalledProcessError):
                pass
            await asyncio.sleep(2)
        raise TimeoutError(f"Android emulator at {self._host} did not boot within {timeout}s")

    async def connect(self) -> None:
        """Connect via ADB and initialize uiautomator2."""
        subprocess.run(
            ["adb", "connect", self._host],
            check=True,
            capture_output=True,
            text=True,
        )
        import uiautomator2 as u2

        self.device = u2.connect(self._host)

    async def install_and_launch(self, apk_path: str, package: str = "com.lelloman.pezzottify") -> None:
        """Install APK and launch the app."""
        subprocess.run(
            ["adb", "-s", self._host, "install", "-r", apk_path],
            check=True,
            capture_output=True,
            text=True,
        )
        self.device.app_start(package)
        await asyncio.sleep(3)

    async def login_password(self, username: str, password: str) -> None:
        """Login via the Android app's password form."""
        # Wait for login screen elements
        self.device(resourceId="com.lelloman.pezzottify:id/username_input").set_text(username)
        self.device(resourceId="com.lelloman.pezzottify:id/password_input").set_text(password)
        self.device(resourceId="com.lelloman.pezzottify:id/login_button").click()
        await asyncio.sleep(3)
