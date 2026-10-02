#!/usr/bin/env python3
"""Inspect complete shell/payload integrity and code separation after a build."""
import argparse
import base64
import hashlib
import io
import json
from pathlib import Path
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--application-id", default="com.lelloman.pezzottify.android.paravoid")
parser.add_argument("output", nargs="?", type=Path, default=Path(__file__).resolve().parent /
                    "app/build/outputs/paravoid/paravoidAndroidPhoneDebug")
args = parser.parse_args()
output = args.output
payload = (output / "payload.vpk").read_bytes()
assert hashlib.sha256(payload).hexdigest() == (output / "payload.sha256").read_text().strip()
with zipfile.ZipFile(output / "shell.apk") as shell:
    assert shell.read("assets/paravoid/payload.vpk") == payload
    policy = json.loads(shell.read("assets/paravoid/shell-policy.json"))
    host = b"".join(shell.read(name) for name in shell.namelist()
                    if name.startswith("classes") and name.endswith(".dex"))
with zipfile.ZipFile(io.BytesIO(payload)) as archive:
    envelope = archive.read("release.json")
    assert envelope == (output / "release.json").read_bytes()
    release = json.loads(base64.b64decode(json.loads(envelope)["body"]))
    assert release["applicationId"] == policy["descriptor"]["installed"]["applicationId"] == args.application_id
    assert release["shellContractId"] == policy["contractId"]
    assert len(archive.namelist()) == len(set(archive.namelist()))
    inventory = release["inventory"]
    assert set(archive.namelist()) == {entry["path"] for entry in inventory} | {"release.json"}
    for entry in inventory:
        content = archive.read(entry["path"])
        assert len(content) == entry["size"], entry["path"]
        assert hashlib.sha256(content).hexdigest() == entry["sha256"], entry["path"]
    assert {"resources.apk", "java-resources.jar", "resource-ledger.json"} <= set(archive.namelist())
    assert any(name.startswith("native/") and name.endswith("/libsimple_assistant.so")
               for name in archive.namelist())
    code = b"".join(archive.read(name) for name in archive.namelist()
                    if name.startswith("code/") and name.endswith(".dex"))
    for component in ("MainActivity", "PezzottifyApplication"):
        descriptor = ("Lcom/lelloman/pezzottify/android/" + component + ";").encode()
        assert descriptor in code and descriptor not in host, component
print("PASS complete shell: embedded payload matches standalone VPK, component integrity and code separation")
