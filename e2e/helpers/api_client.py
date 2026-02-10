"""Password-auth HTTP client for the catalog server API."""

import uuid

import aiohttp


class CatalogApiClient:
    """HTTP client using password authentication with session cookies."""

    def __init__(self, base_url: str):
        self.base_url = base_url.rstrip("/")
        self._session: aiohttp.ClientSession | None = None
        self._device_uuid: str = str(uuid.uuid4())

    async def _ensure_session(self) -> aiohttp.ClientSession:
        if self._session is None or self._session.closed:
            jar = aiohttp.CookieJar(unsafe=True)
            self._session = aiohttp.ClientSession(cookie_jar=jar)
        return self._session

    async def login(
        self, handle: str, password: str, device_uuid: str | None = None
    ) -> dict:
        session = await self._ensure_session()
        self._device_uuid = device_uuid or str(uuid.uuid4())
        async with session.post(
            f"{self.base_url}/v1/auth/login",
            json={
                "user_handle": handle,
                "password": password,
                "device_uuid": self._device_uuid,
                "device_type": "web",
                "device_name": f"E2E API Client {self._device_uuid[:8]}",
            },
        ) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def like_content(self, content_type: str, content_id: str) -> None:
        session = await self._ensure_session()
        async with session.post(
            f"{self.base_url}/v1/user/liked/{content_type}/{content_id}",
        ) as resp:
            resp.raise_for_status()

    async def unlike_content(self, content_type: str, content_id: str) -> None:
        session = await self._ensure_session()
        async with session.delete(
            f"{self.base_url}/v1/user/liked/{content_type}/{content_id}",
        ) as resp:
            resp.raise_for_status()

    async def get_liked_content(self, content_type: str) -> list[str]:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/user/liked/{content_type}",
        ) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def create_playlist(
        self, name: str, track_ids: list[str] | None = None
    ) -> dict:
        session = await self._ensure_session()
        async with session.post(
            f"{self.base_url}/v1/user/playlist",
            json={"name": name, "track_ids": track_ids or []},
        ) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def delete_playlist(self, playlist_id: str) -> None:
        session = await self._ensure_session()
        async with session.delete(
            f"{self.base_url}/v1/user/playlist/{playlist_id}",
        ) as resp:
            resp.raise_for_status()

    async def get_playlists(self) -> list[dict]:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/user/playlists",
        ) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def get_sync_state(self) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/sync/state",
        ) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def get_sync_events(self, since: int) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/sync/events",
            params={"since": since},
        ) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def get_album(self, album_id: str) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/content/album/{album_id}",
        ) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def get_artist(self, artist_id: str) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/content/artist/{artist_id}",
        ) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def get_track(self, track_id: str) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/content/track/{track_id}",
        ) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def search(self, query: str) -> dict:
        session = await self._ensure_session()
        async with session.post(
            f"{self.base_url}/v1/content/search",
            json={"query": query},
        ) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def get_session(self) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/auth/session",
        ) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def close(self) -> None:
        if self._session and not self._session.closed:
            await self._session.close()
