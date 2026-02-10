"""Polling and assertion utilities for E2E tests."""

import asyncio
from typing import Any, Callable, Coroutine


async def wait_until(
    predicate: Callable[[], Coroutine[Any, Any, bool]],
    timeout: float = 10.0,
    interval: float = 0.5,
    message: str = "Condition not met within timeout",
) -> None:
    """Poll an async predicate until it returns True or timeout."""
    deadline = asyncio.get_event_loop().time() + timeout
    last_error = None
    while asyncio.get_event_loop().time() < deadline:
        try:
            result = await predicate()
            if result:
                return
        except Exception as e:
            last_error = e
        await asyncio.sleep(interval)
    raise TimeoutError(f"{message} (last error: {last_error})")


async def assert_eventually(
    check: Callable[[], Coroutine[Any, Any, None]],
    timeout: float = 10.0,
    interval: float = 0.5,
    message: str = "Assertion not satisfied within timeout",
) -> None:
    """Retry an assertion until it passes or timeout.

    The check function should raise AssertionError on failure.
    """
    deadline = asyncio.get_event_loop().time() + timeout
    last_error = None
    while asyncio.get_event_loop().time() < deadline:
        try:
            await check()
            return
        except (AssertionError, Exception) as e:
            last_error = e
        await asyncio.sleep(interval)
    raise TimeoutError(f"{message}: {last_error}")
