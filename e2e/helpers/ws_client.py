"""WebSocket client for monitoring sync events."""

from __future__ import annotations

import asyncio
import json

import websockets


class SyncWebSocketClient:
    """WebSocket client that monitors real-time sync events."""

    def __init__(self, ws_url: str, session_cookie: str):
        self._ws_url = ws_url
        self._session_cookie = session_cookie
        self._ws = None
        self._events: list[dict] = []
        self._listener_task: asyncio.Task | None = None

    async def connect(self) -> None:
        extra_headers = {"Cookie": f"session_token={self._session_cookie}"}
        self._ws = await websockets.connect(
            self._ws_url,
            additional_headers=extra_headers,
        )
        self._listener_task = asyncio.create_task(self._listen())

    async def _listen(self) -> None:
        try:
            async for message in self._ws:
                try:
                    data = json.loads(message)
                    self._events.append(data)
                except json.JSONDecodeError:
                    pass
        except websockets.exceptions.ConnectionClosed:
            pass

    async def wait_for_event(
        self, event_type: str, timeout: float = 5.0
    ) -> dict | None:
        deadline = asyncio.get_event_loop().time() + timeout
        while asyncio.get_event_loop().time() < deadline:
            for event in self._events:
                if event.get("type") == event_type:
                    self._events.remove(event)
                    return event
            await asyncio.sleep(0.1)
        return None

    def get_events(self) -> list[dict]:
        return list(self._events)

    def clear_events(self) -> None:
        self._events.clear()

    async def close(self) -> None:
        if self._listener_task:
            self._listener_task.cancel()
            try:
                await self._listener_task
            except asyncio.CancelledError:
                pass
        if self._ws:
            await self._ws.close()
