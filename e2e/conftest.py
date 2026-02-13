"""Root conftest.py - session-scoped fixtures."""

import pytest

from helpers.config import E2EConfig


@pytest.fixture(scope="session")
def config() -> E2EConfig:
    return E2EConfig.from_env()
