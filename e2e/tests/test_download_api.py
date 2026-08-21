"""Deployed-stack contracts for optional download-manager routes."""

from helpers.api_client import CatalogApiClient
from helpers.async_runner import run_async
from helpers.constants import ADMIN_PASS, ADMIN_USER, TEST_PASS, TEST_USER


class TestDownloadApi:
    def test_enabled_manager_reads_remain_permission_protected(self, config):
        async def _test():
            user = CatalogApiClient(config.server_url)
            admin = CatalogApiClient(config.server_url)
            try:
                await user.login(TEST_USER, TEST_PASS, device_uuid="download-api-user")
                assert await user.download_limits_status() == 403
                assert await user.download_admin_stats_status() == 403

                await admin.login(ADMIN_USER, ADMIN_PASS, device_uuid="download-api-admin")
                assert await admin.download_limits_status() == 200
                assert await admin.download_admin_stats_status() == 200
            finally:
                await user.close()
                await admin.close()

        run_async(_test())
