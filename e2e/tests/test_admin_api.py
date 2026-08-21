"""Deployed-stack contracts for server administration APIs."""

import asyncio

from helpers.api_client import CatalogApiClient
from helpers.async_runner import run_async
from helpers.constants import ADMIN_PASS, ADMIN_USER, TEST_PASS, TEST_USER


class TestAdminApi:
    def test_catalog_sync_exposes_paging_metadata(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            try:
                await api.login(
                    TEST_USER, TEST_PASS, device_uuid="catalog-sync-page-metadata"
                )
                page = await api.get_catalog_sync()
                assert set(page) >= {
                    "events",
                    "current_seq",
                    "has_more",
                    "next_since",
                }
                assert page["has_more"] is False
                assert page["next_since"] == page["current_seq"]
            finally:
                await api.close()

        run_async(_test())

    def test_heavy_catalog_job_lifecycle(self, config):
        async def _test():
            admin = CatalogApiClient(config.server_url)
            try:
                await admin.login(
                    ADMIN_USER, ADMIN_PASS, device_uuid="admin-api-heavy-job"
                )
                jobs = await admin.list_background_jobs()
                assert any(job["id"] == "catalog_cardinality_stats" for job in jobs)

                triggered = await admin.trigger_background_job(
                    "catalog_cardinality_stats"
                )
                assert triggered == {
                    "status": "triggered",
                    "job_id": "catalog_cardinality_stats",
                }

                history = []
                for _ in range(100):
                    history = await admin.get_background_job_history(
                        "catalog_cardinality_stats"
                    )
                    if history and history[0]["status"] != "running":
                        break
                    await asyncio.sleep(0.1)
                assert history[0]["status"] == "completed"

                audit = await admin.get_background_job_audit(
                    "catalog_cardinality_stats"
                )
                completed = next(
                    entry for entry in audit if entry["event_type"] == "completed"
                )
                assert completed["details"]["artists"] > 0
                assert completed["details"]["albums"] > 0
                assert completed["details"]["tracks"] > 0
                assert completed["details"]["mutation_version"] >= 0
            finally:
                await admin.close()

        run_async(_test())

    def test_lightweight_background_job_lifecycle(self, config):
        async def _test():
            admin = CatalogApiClient(config.server_url)
            try:
                await admin.login(
                    ADMIN_USER, ADMIN_PASS, device_uuid="admin-api-background-job"
                )
                jobs = await admin.list_background_jobs()
                assert any(job["id"] == "whatsnew_batch" for job in jobs)

                triggered = await admin.trigger_background_job("whatsnew_batch")
                assert triggered == {
                    "status": "triggered",
                    "job_id": "whatsnew_batch",
                }

                history = []
                for _ in range(50):
                    history = await admin.get_background_job_history("whatsnew_batch")
                    if history and history[0]["status"] != "running":
                        break
                    await asyncio.sleep(0.1)
                assert history[0]["status"] == "completed"

                audit = await admin.get_background_job_audit("whatsnew_batch")
                event_types = {entry["event_type"] for entry in audit}
                assert {"started", "completed"} <= event_types
            finally:
                await admin.close()

        run_async(_test())

    def test_mcp_database_backed_server_stats(self, config):
        async def _test():
            admin = CatalogApiClient(config.server_url)
            try:
                await admin.login(ADMIN_USER, ADMIN_PASS, device_uuid="admin-api-mcp")
                await admin.trigger_background_job("catalog_cardinality_stats")
                for _ in range(100):
                    history = await admin.get_background_job_history(
                        "catalog_cardinality_stats"
                    )
                    if history and history[0]["status"] != "running":
                        break
                    await asyncio.sleep(0.1)
                assert history[0]["status"] == "completed"

                stats = await admin.mcp_server_stats()
                assert stats["catalog"] == {"artists": 2, "albums": 2, "tracks": 5}
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
