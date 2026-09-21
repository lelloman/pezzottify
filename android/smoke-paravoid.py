#!/usr/bin/env python3
"""Logged-out smoke gate on an explicitly selected emulator; installs both APKs.

Never selects a device implicitly or clears app data. Use a disposable emulator
with no accounts configured. Does not test authenticated login or background sync.
"""
import argparse
import json
from pathlib import Path
import re
import sqlite3
import subprocess
import tempfile
import time
import xml.etree.ElementTree as ET

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--serial", required=True)
args = parser.parse_args()
if not re.fullmatch(r"emulator-\d+", args.serial):
    parser.error("Only an explicit emulator serial is accepted; never a phone")
ROOT = Path(__file__).resolve().parent
BASE = "com.lelloman.pezzottify.android"


def adb(*parts, binary=False):
    return subprocess.check_output(["adb", "-s", args.serial, *parts],
                                   text=not binary, timeout=45)


def shell(*parts):
    return adb("shell", *parts).strip()


def login_ui(package):
    for attempt in range(8):
        shell("uiautomator", "dump", "/sdcard/paravoid-smoke.xml")
        root = ET.fromstring(shell("cat", "/sdcard/paravoid-smoke.xml"))
        nodes = [n for n in root.iter("node") if n.get("package") == package]
        texts = {n.get("text") for n in nodes}
        if {"Login", "Sign in with SSO"} <= texts:
            return
        time.sleep(1)
    raise AssertionError(f"{package}: login UI missing")


assert shell("getprop", "ro.kernel.qemu") == "1"
for attempt in range(30):
    if shell("getprop", "sys.boot_completed") == "1":
        break
    time.sleep(1)
else:
    raise AssertionError("Emulator did not finish booting")
print("Emulator API", shell("getprop", "ro.build.version.sdk"), flush=True)
for mode in ("normal", "paravoidAndroid"):
    output = ROOT / f"app/build/outputs/apk/{mode}Phone/debug"
    metadata = json.loads((output / "output-metadata.json").read_text())
    assert metadata["applicationId"] == BASE + (".paravoid" if mode == "paravoidAndroid" else "")
    print(adb("install", "-r", str(output / metadata["elements"][0]["outputFile"])).strip())

for package in (BASE, BASE + ".paravoid"):
    uri = package + "://oauth/callback"
    matches = shell("cmd", "package", "query-activities", "--brief", "-a",
                    "android.intent.action.VIEW", "-c", "android.intent.category.BROWSABLE", "-d", uri)
    assert "1 activities found:" in matches and package + "/" in matches, matches
    component = package + "/" + ("com.lelloman.paravoidandroid.runtime.LauncherActivity"
                                 if package.endswith(".paravoid") else BASE + ".MainActivity")
    previous = None
    for stage in ("cold", "restart"):
        shell("am", "force-stop", package)
        launched = shell("am", "start", "-W", "-n", component)
        assert "Status: ok" in launched, launched
        login_ui(package)
        pid = shell("pidof", package)
        assert pid and pid != previous
        previous = pid
        logs = adb("logcat", "-d", "--pid=" + pid)
        assert "Androidoscopy initialized successfully" in logs
        assert "FATAL EXCEPTION" not in logs and "Payload initialization failed" not in logs
        print(f"PASS {package}: {stage} login UI + Androidoscopy initialization", flush=True)

    acceleration = shell("settings", "get", "system", "accelerometer_rotation")
    rotation = shell("settings", "get", "system", "user_rotation")
    try:
        shell("settings", "put", "system", "accelerometer_rotation", "0")
        shell("settings", "put", "system", "user_rotation", "1")
        login_ui(package)
    finally:
        for name, value in (("user_rotation", rotation), ("accelerometer_rotation", acceleration)):
            if value == "null":
                shell("settings", "delete", "system", name)
            else:
                shell("settings", "put", "system", name, value)
    login_ui(package)
    # No fabricated auth code: exercise a rejected/cancelled callback's routing only.
    callback = shell("am", "start", "-W", "-a", "android.intent.action.VIEW", "-c",
                     "android.intent.category.BROWSABLE", "-d", uri)
    assert "Status: ok" in callback, callback
    login_ui(package)
    shell("am", "force-stop", package)
    files = shell("run-as", package, "ls", "databases").splitlines()
    assert {"StaticsDb", "user_content"} <= set(files)
    with tempfile.TemporaryDirectory(prefix="pezzottify-paravoid-db-") as folder:
        for name in files:
            assert "/" not in name and name not in (".", "..")
            (Path(folder) / name).write_bytes(adb("exec-out", "run-as", package, "cat", "databases/" + name, binary=True))
        for name in ("StaticsDb", "user_content"):
            connection = sqlite3.connect(str(Path(folder) / name))
            try:
                assert connection.execute("PRAGMA integrity_check").fetchone() == ("ok",)
                assert connection.execute("SELECT identity_hash FROM room_master_table WHERE id=42").fetchone()
            finally:
                connection.close()
    print(f"PASS {package}: rotation, callback routing, two Room databases", flush=True)
print("PASS logged-out smoke gate. SSO, library Activity UI and authenticated work remain separate gates.")
