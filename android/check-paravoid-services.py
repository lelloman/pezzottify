#!/usr/bin/env python3
"""Fail if a shrunk Paravoid payload broke a java.util.ServiceLoader entry.

Every interface and provider named in the payload's META-INF/services must survive
payload R8 under its original name; otherwise ServiceLoader finds nothing (or a
missing class) at runtime, as the 0.5.1758 startup crash showed.
Usage: check-paravoid-services.py <paravoid variant output directory>
"""
import re
import sys
import zipfile
from pathlib import Path

out = Path(sys.argv[1])
mapping = out / "payload-mapping.txt"
if not mapping.exists():
    print("PASS services: payload not shrunk (no payload-mapping.txt)")
    sys.exit(0)
renamed = {}
for line in mapping.read_text().splitlines():
    m = re.match(r"^(\S+) -> (\S+):$", line)
    if m:
        renamed[m.group(1)] = m.group(2)
kept_classes = set(renamed)
needed = []
with zipfile.ZipFile(out / "java-resources.jar") as jar:
    for name in jar.namelist():
        if name.startswith("META-INF/services/") and not name.endswith("/"):
            needed.append(name.rsplit("/", 1)[1])
            for line in jar.read(name).decode().splitlines():
                provider = line.split("#")[0].strip()
                if provider:
                    needed.append(provider)
problems = []
for cls in needed:
    if cls not in kept_classes:
        problems.append(f"{cls}: removed by payload R8")
    elif renamed[cls] != cls:
        problems.append(f"{cls}: renamed to {renamed[cls]}")
if problems:
    print("FAIL services: payload R8 broke ServiceLoader entries:")
    for problem in problems:
        print("  " + problem)
    sys.exit(1)
print(f"PASS services: {len(needed)} ServiceLoader classes kept under their names")
