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

    def _csrf_headers(self) -> dict[str, str]:
        """Return the double-submit token required for cookie-authenticated writes."""
        if self._session is None:
            return {}
        cookies = self._session.cookie_jar.filter_cookies(self.base_url)
        csrf_cookie = cookies.get("csrf_token")
        return {"x-csrf-token": csrf_cookie.value} if csrf_cookie else {}

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
            return await resp.json(content_type=None)

    async def ingestion_jobs_status(self) -> int:
        """Return the status of the optional ingestion job-list endpoint."""
        session = await self._ensure_session()
        async with session.get(f"{self.base_url}/v1/ingestion/my-jobs") as resp:
            await resp.read()
            return resp.status

    async def like_content(self, content_type: str, content_id: str) -> None:
        session = await self._ensure_session()
        async with session.post(
            f"{self.base_url}/v1/user/liked/{content_type}/{content_id}",
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()

    async def unlike_content(self, content_type: str, content_id: str) -> None:
        session = await self._ensure_session()
        async with session.delete(
            f"{self.base_url}/v1/user/liked/{content_type}/{content_id}",
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()

    async def get_liked_content(self, content_type: str) -> list[str]:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/user/liked/{content_type}",
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def get_user_settings(self) -> dict:
        session = await self._ensure_session()
        async with session.get(f"{self.base_url}/v1/user/settings") as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def update_user_settings(self, settings: list[dict]) -> None:
        session = await self._ensure_session()
        async with session.put(
            f"{self.base_url}/v1/user/settings",
            json={"settings": settings},
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()

    async def get_user_devices(self) -> dict:
        session = await self._ensure_session()
        async with session.get(f"{self.base_url}/v1/user/devices") as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def submit_bug_report(self, title: str, description: str) -> str:
        session = await self._ensure_session()
        async with session.post(
            f"{self.base_url}/v1/user/bug-report",
            json={
                "title": title,
                "description": description,
                "client_type": "docker-e2e",
            },
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()
            body = await resp.json(content_type=None)
            return body["id"]

    async def get_admin_bug_report(self, report_id: str) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/admin/bug-report/{report_id}"
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def delete_admin_bug_report(self, report_id: str) -> None:
        session = await self._ensure_session()
        async with session.delete(
            f"{self.base_url}/v1/admin/bug-report/{report_id}",
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()

    async def create_show_draft(self, brief: str) -> dict:
        session = await self._ensure_session()
        async with session.post(
            f"{self.base_url}/v1/content/admin/shows/drafts",
            json={"brief": brief, "target_duration_minutes": 30, "language": "en"},
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def get_admin_shows(self) -> list[dict]:
        session = await self._ensure_session()
        async with session.get(f"{self.base_url}/v1/content/admin/shows") as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def delete_admin_show(self, show_id: str) -> None:
        session = await self._ensure_session()
        async with session.delete(
            f"{self.base_url}/v1/content/admin/shows/{show_id}",
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()

    async def prepare_backup(self) -> dict:
        session = await self._ensure_session()
        async with session.post(
            f"{self.base_url}/v1/admin/backup/prepare",
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def download_limits_status(self) -> int:
        session = await self._ensure_session()
        async with session.get(f"{self.base_url}/v1/download/limits") as resp:
            return resp.status

    async def download_admin_stats_status(self) -> int:
        session = await self._ensure_session()
        async with session.get(f"{self.base_url}/v1/download/admin/stats") as resp:
            return resp.status

    async def create_playlist(
        self, name: str, track_ids: list[str] | None = None
    ) -> str:
        session = await self._ensure_session()
        async with session.post(
            f"{self.base_url}/v1/user/playlist",
            json={"name": name, "track_ids": track_ids or []},
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def delete_playlist(self, playlist_id: str) -> None:
        session = await self._ensure_session()
        async with session.delete(
            f"{self.base_url}/v1/user/playlist/{playlist_id}",
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()

    async def get_playlists(self) -> list[str]:
        """Get list of playlist IDs for the current user."""
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/user/playlists",
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def get_playlist(self, playlist_id: str) -> dict:
        """Get a single playlist by ID with full details."""
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/user/playlist/{playlist_id}",
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def try_add_tracks_to_playlist(
        self, playlist_id: str, track_ids: list[str]
    ) -> tuple[int, dict]:
        """Add tracks and return the response contract without raising on errors."""
        session = await self._ensure_session()
        async with session.put(
            f"{self.base_url}/v1/user/playlist/{playlist_id}/add",
            json={"tracks_ids": track_ids},
            headers=self._csrf_headers(),
        ) as resp:
            return resp.status, await resp.json(content_type=None)

    async def get_sync_state(self) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/sync/state",
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def get_sync_events(self, since: int) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/sync/events",
            params={"since": since},
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def get_album(self, album_id: str) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/content/album/{album_id}",
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def get_artist(self, artist_id: str) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/content/artist/{artist_id}",
        ) as resp:
            resp.raise_for_status()
            data = await resp.json(content_type=None)
            # Unwrap {"artist": {...}, "related_artists": [...]}
            return data.get("artist", data) if isinstance(data, dict) else data

    async def get_track(self, track_id: str) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/content/track/{track_id}",
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def search(self, query: str) -> dict:
        session = await self._ensure_session()
        async with session.post(
            f"{self.base_url}/v1/content/search",
            json={"query": query},
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def continuation_recommendations(
        self, context_track_ids: list[str], count: int = 1
    ) -> dict:
        session = await self._ensure_session()
        async with session.post(
            f"{self.base_url}/v1/content/recommendations/continuation",
            json={
                "context_track_ids": context_track_ids,
                "exclude_track_ids": [],
                "count": count,
            },
            headers=self._csrf_headers(),
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def radio(self, entity_type: str, entity_id: str, count: int = 10) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/content/radio/{entity_type}/{entity_id}",
            params={"count": count},
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def get_session(self) -> dict:
        session = await self._ensure_session()
        async with session.get(
            f"{self.base_url}/v1/auth/session",
        ) as resp:
            resp.raise_for_status()
            return await resp.json(content_type=None)

    async def session_status(self) -> int:
        """Return the session endpoint status without raising for an expired session."""
        session = await self._ensure_session()
        async with session.get(f"{self.base_url}/v1/auth/session") as resp:
            return resp.status

    async def logout(self) -> int:
        """Revoke the current session and return the response status."""
        session = await self._ensure_session()
        async with session.post(
            f"{self.base_url}/v1/auth/logout",
            headers=self._csrf_headers(),
        ) as resp:
            return resp.status

    async def close(self) -> None:
        if self._session and not self._session.closed:
            await self._session.close()
