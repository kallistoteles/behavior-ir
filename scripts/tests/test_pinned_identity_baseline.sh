#!/usr/bin/env bash
# Feature 507 (research R3 requirement 7): the established model identities never change. The
# compat gate checks every established tuple; this check also compares every receipt the
# reference cases produce (ModelHash, BehaviorHash, canonical document, source map) with the
# baseline captured before the transition-binding extension. Every baseline receipt must still
# be produced byte-for-byte after normalizing only top-level Core release metadata;
# new receipts (new tuples) may be added. A failure means stop and
# investigate: never edit the fixture or the baseline to make it pass.
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
baseline="$here/specs/507-work-queue/baseline/compat-print.txt"
"$here/scripts/check-models-compat.py" >/dev/null
"$here/scripts/check-models-compat.py" --print | python3 -c '
import json, sys
from collections import Counter
def canon(receipt):
    receipt = dict(receipt)
    receipt.pop("core_release", None)
    return json.dumps(receipt, sort_keys=True, separators=(",", ":"))
base = Counter(map(canon, json.load(open(sys.argv[1]))))
now = Counter(map(canon, json.load(sys.stdin)))
missing = base - now
if missing:
    for r in missing:
        d = json.loads(r)
        print("changed or missing receipt:", d.get("model_kind"), d.get("model_hash"), file=sys.stderr)
    sys.exit(1)
print(f"test_pinned_identity_baseline: OK ({sum(base.values())} baseline receipts unchanged, "
      f"{sum((now - base).values())} new)")
' "$baseline"
