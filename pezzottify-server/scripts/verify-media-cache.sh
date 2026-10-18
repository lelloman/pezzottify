#!/usr/bin/env bash
# Linux syscall audit of original media reads AND metadata/existence probes.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
artifact=$(cargo test --features fast --lib --no-run --message-format=json | python3 -c '
import json,sys
for line in sys.stdin:
    item=json.loads(line)
    if item.get("reason")=="compiler-artifact" and item.get("executable") and "lib" in item["target"]["kind"]:
        print(item["executable"])
')
trace=$(mktemp)
trap 'rm -f "$trace"' EXIT
"${STRACE:-strace}" -f -yy -o "$trace" -e trace=%file,read,pread64,lseek,fstat "$artifact" \
    media::filesystem_cache_tests::representative_audio_seek_artwork_workload --exact --nocapture
python3 - "$trace" <<'PY'
import re,sys
phase=None
counts={"cold":0,"warm":0}
root=None
for line in open(sys.argv[1]):
    marker=re.search(r'"([^" ]+)/\.cache-(cold|warm)-(start|end)"',line)
    if marker:
        root=marker[1]
        phase=marker[2] if marker[3]=="start" else None
        continue
    if phase and root and re.search(re.escape(root)+r'/media(?:/|[">])',line):
        counts[phase]+=1
        if phase=="warm":
            print("Unexpected original-media syscall: "+line.rstrip(),file=sys.stderr)
assert counts["cold"]>0, "audit did not observe original media during cold workload"
assert counts["warm"]==0, "warm cache probed original media"
print(f'Original-media syscalls (including metadata/existence and byte reads): cold={counts["cold"]}, warm={counts["warm"]}')
PY
