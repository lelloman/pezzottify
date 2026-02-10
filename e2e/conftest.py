"""Root conftest.py - session-scoped fixtures."""

import pytest
import pytest_asyncio

from helpers.config import E2EConfig


@pytest.fixture(scope="session")
def config() -> E2EConfig:
    return E2EConfig.from_env()


@pytest_asyncio.fixture(scope="session")
async def admin_api(config: E2EConfig):
    from helpers.api_client import CatalogApiClient

    client = CatalogApiClient(config.catalog_server_url)
    await client.login(config.admin_user, config.admin_pass)
    yield client
    await client.close()


@pytest_asyncio.fixture(scope="session")
async def user_api(config: E2EConfig):
    from helpers.api_client import CatalogApiClient

    client = CatalogApiClient(config.catalog_server_url)
    await client.login(config.test_user, config.test_pass)
    yield client
    await client.close()
