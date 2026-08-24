"""Small deployed-stack load budget for independent backend work lanes."""

import asyncio
import math
import time

from helpers.api_client import CatalogApiClient
from helpers.async_runner import run_async
from helpers.constants import ADMIN_PASS, ADMIN_USER, TEST_PASS, TEST_USER, TRACK_1_ID


AUTH_REQUESTS = 4
REQUESTS_PER_INTERACTIVE_PATH = 8
INTERACTIVE_P95_BUDGET_SECONDS = 2.0
AUTH_P95_BUDGET_SECONDS = 10.0
TOTAL_BUDGET_SECONDS = 20.0


def _p95(samples: list[float]) -> float:
    ordered = sorted(samples)
    return ordered[max(0, math.ceil(len(ordered) * 0.95) - 1)]


class TestMixedWorkload:
    def test_auth_stream_ingestion_and_sync_stay_within_budgets(self, config):
        async def _test():
            admin = CatalogApiClient(config.server_url)
            login_clients: list[CatalogApiClient] = []
            samples: dict[str, list[float]] = {
                "authentication": [],
                "streaming": [],
                "ingestion": [],
                "synchronization": [],
            }

            async def timed(workload: str, operation):
                started = time.perf_counter()
                await operation()
                samples[workload].append(time.perf_counter() - started)

            async def login(index: int):
                client = CatalogApiClient(config.server_url)
                login_clients.append(client)
                await client.login(
                    TEST_USER,
                    TEST_PASS,
                    device_uuid=f"mixed-workload-login-{index}",
                )

            async def stream():
                body = await admin.stream_track_range(TRACK_1_ID, "bytes=0-31")
                assert len(body) == 32

            async def ingestion():
                # The deployed E2E configuration intentionally disables ingestion;
                # the Rust mixed-workload test covers the enabled database path.
                assert await admin.ingestion_jobs_status() == 503

            async def synchronization():
                state = await admin.get_sync_state()
                assert isinstance(state, dict)

            try:
                await admin.login(
                    ADMIN_USER, ADMIN_PASS, device_uuid="mixed-workload-admin"
                )
                operations = [
                    timed("authentication", lambda index=index: login(index))
                    for index in range(AUTH_REQUESTS)
                ]
                for _ in range(REQUESTS_PER_INTERACTIVE_PATH):
                    operations.extend(
                        [
                            timed("streaming", stream),
                            timed("ingestion", ingestion),
                            timed("synchronization", synchronization),
                        ]
                    )

                started = time.perf_counter()
                await asyncio.wait_for(
                    asyncio.gather(*operations), timeout=TOTAL_BUDGET_SECONDS
                )
                total = time.perf_counter() - started

                assert total <= TOTAL_BUDGET_SECONDS
                assert _p95(samples["authentication"]) <= AUTH_P95_BUDGET_SECONDS
                for workload in ["streaming", "ingestion", "synchronization"]:
                    assert _p95(samples[workload]) <= INTERACTIVE_P95_BUDGET_SECONDS
            finally:
                await asyncio.gather(
                    *(client.close() for client in login_clients),
                    return_exceptions=True,
                )
                await admin.close()

        run_async(_test())
