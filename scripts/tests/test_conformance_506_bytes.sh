#!/usr/bin/env bash
# Feature 507 (FR-037, SC-008): sharing the conformance infrastructure never changes feature
# 506's report. The quick-mode 506 report must stay byte-identical to the baseline taken before
# the change. Inside scripts/gates.sh the conformance gate has just written it; otherwise the
# 506 suite is run first.
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
report="$here/target/conformance/506-report.json"
baseline="$here/specs/507-work-queue/baseline/506-report-quick.json"
if [ ! -f "$report" ] || ! grep -q '"mode":"quick"' "$report"; then
  "$here/scripts/conformance.sh" 506 quick >/dev/null
fi
if cmp -s "$report" "$baseline"; then
  echo "test_conformance_506_bytes: OK"
else
  echo "test_conformance_506_bytes: the 506 report differs from the baseline" >&2
  diff <(python3 -m json.tool "$baseline") <(python3 -m json.tool "$report") | head -20 >&2
  exit 1
fi
