#!/usr/bin/env python3
"""Read-only build gate. Run after assembling both phone/debug variants."""
import io
import json
from pathlib import Path
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parent / "app/build"
A = "{http://schemas.android.com/apk/res/android}"
BASE = "com.lelloman.pezzottify.android"
RUNTIME = "com.lelloman.paravoidandroid.runtime."
ACTIVITIES = (
    BASE + ".MainActivity",
    "net.openid.appauth.AuthorizationManagementActivity",
    "com.lelloman.androidoscopy.ui.DashboardActivity",
    "com.lelloman.androidoscopy.ui.SessionActivity",
)


def dex(archive):
    return b"".join(archive.read(n) for n in archive.namelist()
                    if n.startswith("classes") and n.endswith(".dex"))


def descriptor(name):
    return ("L" + name.replace(".", "/") + ";").encode()


for mode in ("normal", "paravoidAndroid"):
    shell = mode == "paravoidAndroid"
    variant = mode + "PhoneDebug"
    cap = variant[0].upper() + variant[1:]
    expected_id = BASE + (".paravoid" if shell else "")
    output = ROOT / f"outputs/apk/{mode}Phone/debug"
    metadata = json.loads((output / "output-metadata.json").read_text())
    assert metadata["applicationId"] == expected_id
    manifest = ET.parse(ROOT / f"intermediates/packaged_manifests/{variant}/process{cap}ManifestForPackage/AndroidManifest.xml").getroot()
    assert manifest.get("package") == expected_id
    assert manifest.find("uses-sdk").get(A + "minSdkVersion") == ("30" if shell else "24")
    app = manifest.find("application")
    assert app.get(A + "name") == (RUNTIME + "ShellApplication" if shell else BASE + ".PezzottifyApplication")
    activities = {n.get(A + "name"): n for n in app.findall("activity")}
    assert set(ACTIVITIES) <= activities.keys()
    assert "net.openid.appauth.RedirectUriReceiverActivity" not in activities
    schemes = {n.get(A + "scheme") for n in activities[BASE + ".MainActivity"].iter("data")}
    assert expected_id in schemes
    if shell:
        assert BASE not in schemes
        assert RUNTIME + "LauncherActivity" in activities
    authorities = {n.get(A + "authorities") for n in app.findall("provider")}
    assert expected_id + ".fileprovider" in authorities
    with zipfile.ZipFile(output / metadata["elements"][0]["outputFile"]) as apk:
        host = dex(apk)
        if shell:
            with zipfile.ZipFile(io.BytesIO(apk.read("assets/paravoid/module.zip"))) as payload:
                implementation = dex(payload)
                count = sum(n.endswith(".dex") for n in payload.namelist())
                print(f"Payload: {count} DEX files, {len(implementation)} bytes")
            assert descriptor(RUNTIME + "ShellApplication") in host
        else:
            implementation = host
            assert "assets/paravoid/module.zip" not in apk.namelist()
            assert descriptor(RUNTIME + "ShellApplication") not in host
        for name in (*ACTIVITIES, BASE + ".PezzottifyApplication"):
            assert descriptor(name) in implementation, (mode, name)
            if shell:
                assert descriptor(name) not in host, name
        assert "lib/x86_64/libsimple_assistant.so" in apk.namelist()
    print(f"PASS: {variant}: identity, callback, minimum SDK, component declarations and DEX ownership")
