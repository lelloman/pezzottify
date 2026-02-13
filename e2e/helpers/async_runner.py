"""Helper for running async code from sync test contexts.

Playwright's sync API keeps a background event loop running, which prevents
asyncio.run() from working in the same thread. This module provides a helper
that runs async code in a separate thread with its own event loop.
"""

import asyncio
import threading


def run_async(coro):
    """Run an async coroutine from sync code, even when an event loop exists.

    Uses a separate thread to avoid conflicts with Playwright's internal
    event loop or pytest-asyncio's loop.
    """
    result = [None]
    error = [None]

    def _target():
        try:
            result[0] = asyncio.run(coro)
        except BaseException as e:
            error[0] = e

    t = threading.Thread(target=_target)
    t.start()
    t.join(timeout=60)

    if t.is_alive():
        raise TimeoutError("Async operation timed out after 60s")
    if error[0] is not None:
        raise error[0]
    return result[0]
