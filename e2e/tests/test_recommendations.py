"""System contracts for user-facing recommendation reads."""

import pytest

from helpers.api_client import CatalogApiClient
from helpers.async_runner import run_async
from helpers.constants import TEST_PASS, TEST_USER, TRACK_1_ID


pytestmark = pytest.mark.web


class TestRecommendationsApi:
    def test_continuation_without_embeddings_is_empty(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid="recommend-continuation")
                data = await api.continuation_recommendations([TRACK_1_ID], count=5)
                assert data == {"track_ids": []}
            finally:
                await api.close()

        run_async(_test())

    def test_track_radio_without_embeddings_returns_seed(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid="recommend-radio")
                data = await api.radio("track", TRACK_1_ID, count=10)
                assert data == {"track_ids": [TRACK_1_ID]}
            finally:
                await api.close()

        run_async(_test())
