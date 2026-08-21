"""Deployed-stack contracts for server administration APIs."""

from helpers.api_client import CatalogApiClient
from helpers.async_runner import run_async
from helpers.constants import ADMIN_PASS, ADMIN_USER, TEST_PASS, TEST_USER


class TestAdminApi:
    def test_mcp_database_backed_server_stats(self, config):
        async def _test():
            admin = CatalogApiClient(config.server_url)
            try:
                await admin.login(ADMIN_USER, ADMIN_PASS, device_uuid="admin-api-mcp")
                stats = await admin.mcp_server_stats()
                # The seed importer bypasses catalog cardinality triggers, so MCP's
                # cached counters currently remain zero in the deployed fixture.
                assert stats["catalog"] == {"artists": 0, "albums": 0, "tracks": 0}
                assert stats["users"]["total_users"] == 2
            finally:
                await admin.close()

        run_async(_test())

    def test_optional_ingestion_api_reports_disabled_service(self, config):
        async def _test():
            admin = CatalogApiClient(config.server_url)
            try:
                await admin.login(
                    ADMIN_USER, ADMIN_PASS, device_uuid="admin-api-ingestion"
                )
                assert await admin.ingestion_jobs_status() == 503
            finally:
                await admin.close()

        run_async(_test())

    def test_show_draft_lifecycle(self, config):
        async def _test():
            admin = CatalogApiClient(config.server_url)
            show_id = None
            try:
                await admin.login(ADMIN_USER, ADMIN_PASS, device_uuid="admin-api-show")
                show = await admin.create_show_draft("Docker E2E catalog tour")
                show_id = show["id"]
                assert show["status"] == "script_ready"
                assert any(
                    item["id"] == show_id for item in await admin.get_admin_shows()
                )
            finally:
                if show_id is not None:
                    await admin.delete_admin_show(show_id)
                await admin.close()

        run_async(_test())

    def test_bug_report_is_visible_to_admin(self, config):
        async def _test():
            user = CatalogApiClient(config.server_url)
            admin = CatalogApiClient(config.server_url)
            report_id = None
            try:
                await user.login(TEST_USER, TEST_PASS, device_uuid="admin-api-report-user")
                report_id = await user.submit_bug_report(
                    "Docker E2E report", "A deployed-stack characterization report"
                )

                await admin.login(
                    ADMIN_USER, ADMIN_PASS, device_uuid="admin-api-report-admin"
                )
                report = await admin.get_admin_bug_report(report_id)
                assert report["title"] == "Docker E2E report"
                assert report["user_handle"] == TEST_USER
            finally:
                if report_id is not None:
                    await admin.delete_admin_bug_report(report_id)
                await user.close()
                await admin.close()

        run_async(_test())

    def test_backup_prepare_reports_registered_databases(self, config):
        async def _test():
            admin = CatalogApiClient(config.server_url)
            try:
                await admin.login(ADMIN_USER, ADMIN_PASS, device_uuid="admin-api-backup")
                result = await admin.prepare_backup()
                assert result["all_succeeded"] is True
                assert len(result["databases"]) >= 4
                assert all(database["success"] for database in result["databases"])
            finally:
                await admin.close()

        run_async(_test())
