"""E2E test configuration from environment variables."""

import os
from dataclasses import dataclass, field

from .constants import ADMIN_PASS, ADMIN_USER, TEST_PASS, TEST_USER


@dataclass
class E2EConfig:
    catalog_server_url: str
    oidc_url: str
    web_url: str
    android_hosts: list[str]
    test_user: str = TEST_USER
    test_pass: str = TEST_PASS
    admin_user: str = ADMIN_USER
    admin_pass: str = ADMIN_PASS

    @classmethod
    def from_env(cls) -> "E2EConfig":
        catalog_server_url = os.environ.get(
            "CATALOG_SERVER_URL", "http://catalog-server:3001"
        )
        oidc_url = os.environ.get("OIDC_URL", "http://mock-oidc:8080")
        web_url = catalog_server_url  # frontend served by catalog-server
        android_hosts_str = os.environ.get("ANDROID_HOSTS", "")
        android_hosts = [
            h.strip() for h in android_hosts_str.split(",") if h.strip()
        ]
        return cls(
            catalog_server_url=catalog_server_url,
            oidc_url=oidc_url,
            web_url=web_url,
            android_hosts=android_hosts,
        )
