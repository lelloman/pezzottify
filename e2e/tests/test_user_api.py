"""System-level contracts for user-owned API state."""

from helpers.api_client import CatalogApiClient
from helpers.async_runner import run_async
from helpers.constants import TEST_PASS, TEST_USER, TRACK_1_ID


class TestUserApi:
    """Exercise user state through the deployed HTTP stack."""

    def test_liked_track_lifecycle_updates_sync_state(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid="user-api-liked")
                await api.unlike_content("track", TRACK_1_ID)
                before = await api.get_sync_state()

                await api.like_content("track", TRACK_1_ID)

                assert TRACK_1_ID in await api.get_liked_content("track")
                after_like = await api.get_sync_state()
                assert after_like["seq"] > before["seq"]

                await api.unlike_content("track", TRACK_1_ID)
                assert TRACK_1_ID not in await api.get_liked_content("track")
                after_unlike = await api.get_sync_state()
                assert after_unlike["seq"] > after_like["seq"]
            finally:
                await api.close()

        run_async(_test())

    def test_settings_round_trip(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            original_notify = False
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid="user-api-settings")
                original = await api.get_user_settings()
                original_settings = original["settings"]
                original_notify = next(
                    (
                        setting["value"]
                        for setting in original_settings
                        if setting["key"] == "notify_whatsnew"
                    ),
                    False,
                )
                updated_notify = not original_notify

                await api.update_user_settings(
                    [{"key": "notify_whatsnew", "value": updated_notify}]
                )
                updated = await api.get_user_settings()
                assert {
                    "key": "notify_whatsnew",
                    "value": updated_notify,
                } in updated["settings"]
            finally:
                try:
                    await api.update_user_settings(
                        [{"key": "notify_whatsnew", "value": original_notify}]
                    )
                finally:
                    await api.close()

        run_async(_test())

    def test_login_device_is_returned_with_default_policy(self, config):
        async def _test():
            api = CatalogApiClient(config.server_url)
            device_uuid = "user-api-device"
            try:
                await api.login(TEST_USER, TEST_PASS, device_uuid=device_uuid)

                response = await api.get_user_devices()
                device = next(
                    item
                    for item in response["devices"]
                    if item["device_uuid"] == device_uuid
                )
                assert device["device_type"] == "web"
                assert device["share_policy"] == {
                    "mode": "deny_everyone",
                    "allow_users": [],
                    "allow_roles": [],
                    "deny_users": [],
                }
            finally:
                await api.close()

        run_async(_test())
