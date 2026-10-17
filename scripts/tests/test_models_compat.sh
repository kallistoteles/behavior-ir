#!/usr/bin/env bash
# Tests of scripts/check-models-compat.py (feature 501, SC-009, lowering contract §7): an
# established version tuple's hashes are immutable. Editing an expected hash never passes; a
# changed lowering passes only under a new tuple; only tuples of an unpinned Core may go.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0

tuple() { # tuple <lowerer> <core version> <cases json>
  printf '{"core_release":{"commit":"c%s","version":"%s"},"lowerer_version":%s,"model_kind":"state_machine","model_semantic_version":1,"cases":%s}' \
    "$2" "$2" "$1" "$3"
}
manifest() { # manifest <tuple>...
  local IFS=,
  printf '{"format":"behavior.model_compat.v1","tuples":[%s]}\n' "$*"
}
computed() { # computed <lowerer> <cases json>
  printf '{"tuple":{"core_release":{"commit":"c1.0","version":"1.0"},"lowerer_version":%s,"model_kind":"state_machine","model_semantic_version":1},"cases":%s}\n' \
    "$1" "$2"
}
A1='{"a":{"behavior_hash":"sha256:b1","models":{"m":"sha256:m1"}}}'
A2='{"a":{"behavior_hash":"sha256:b2","models":{"m":"sha256:m2"}}}'
AB='{"a":{"behavior_hash":"sha256:b1","models":{"m":"sha256:m1"}},"b":{"behavior_hash":"sha256:b3","models":{"n":"sha256:n3"}}}'

# repo <name> <base manifest>: a repository whose `main` holds the base manifest, on a branch.
repo() {
  local r="$tmp/$1"
  mkdir -p "$r/models/fixtures"
  git -C "$r" init -q -b main
  printf '{"commit":"c1.0","version":"1.0"}\n' >"$r/core-release.json"
  printf '%s' "$2" >"$r/models/fixtures/compat.json"
  git -C "$r" add -A
  git -C "$r" -c user.name=t -c user.email=t@t commit -qm base
  git -C "$r" checkout -qb feature
  echo "$r"
}

# expect <PASS|FAIL> <name> <repo> <branch manifest> <computed>
expect() {
  local r="$3"
  printf '%s' "$4" >"$r/models/fixtures/compat.json"
  printf '%s' "$5" >"$tmp/computed.json"
  if python3 "$here/check-models-compat.py" --root "$r" --computed "$tmp/computed.json" \
      >/dev/null 2>"$tmp/err"; then got=PASS; else got=FAIL; fi
  if [ -n "${VERBOSE:-}" ]; then echo "$2: $got $(cat "$tmp/err")" >&2; fi
  if [ "$got" != "$1" ]; then
    echo "test_models_compat: $2: expected $1, got $got: $(cat "$tmp/err")" >&2
    failures=$((failures + 1))
  fi
}

base="$(manifest "$(tuple 1 1.0 "$A1")")"
expect PASS "unchanged" "$(repo unchanged "$base")" "$base" "$(computed 1 "$A1")"
expect FAIL "edited expected hash, same tuple" "$(repo edited "$base")" \
  "$(manifest "$(tuple 1 1.0 "$A2")")" "$(computed 1 "$A2")"
expect FAIL "output changed, manifest not edited" "$(repo drift "$base")" "$base" "$(computed 1 "$A2")"
expect PASS "lowerer bump with a new tuple" "$(repo bump "$base")" \
  "$(manifest "$(tuple 1 1.0 "$A1")" "$(tuple 2 1.0 "$A2")")" "$(computed 2 "$A2")"

base_ab="$(manifest "$(tuple 1 1.0 "$AB")")"
expect FAIL "removed case of an established tuple" "$(repo removed "$base_ab")" "$base" "$(computed 1 "$A1")"

old="$(manifest "$(tuple 1 0.9 "$A2")" "$(tuple 1 1.0 "$A1")")"
expect PASS "tuple of an unpinned core removed" "$(repo unpinned "$old")" "$base" "$(computed 1 "$A1")"
expect FAIL "tuple of the pinned core removed" "$(repo pinned "$base")" \
  "$(manifest "$(tuple 2 1.0 "$A2")")" "$(computed 2 "$A2")"

# Feature 502: a RequestFlow tuple beside the StateMachine tuple (research R10).
rf_tuple() { # rf_tuple <embedded lowerer> <cases json>
  printf '{"core_release":{"commit":"c1.0","version":"1.0"},"embedded":[{"kind":"state_machine","lowerer_version":%s,"model_semantic_version":1}],"lowerer_version":1,"model_kind":"request_flow","model_semantic_version":1,"cases":%s}' \
    "$1" "$2"
}
groups() { # groups <sm lowerer> <sm cases> <embedded lowerer> <rf cases>
  printf '{"groups":[{"tuple":{"core_release":{"commit":"c1.0","version":"1.0"},"lowerer_version":%s,"model_kind":"state_machine","model_semantic_version":1},"cases":%s},{"tuple":{"core_release":{"commit":"c1.0","version":"1.0"},"embedded":[{"kind":"state_machine","lowerer_version":%s,"model_semantic_version":1}],"lowerer_version":1,"model_kind":"request_flow","model_semantic_version":1},"cases":%s}]}\n' \
    "$1" "$2" "$3" "$4"
}
R1='{"rf":{"behavior_hash":"sha256:r1","embedded":{"f":"sha256:e1"},"entities":{"f":"F"},"models":{"f":"sha256:f1"}}}'
R2='{"rf":{"behavior_hash":"sha256:r2","embedded":{"f":"sha256:e2"},"entities":{"f":"F"},"models":{"f":"sha256:f2"}}}'
both="$(manifest "$(tuple 1 1.0 "$A1")" "$(rf_tuple 1 "$R1")")"
expect PASS "both tuples, unchanged" "$(repo both "$base")" "$both" "$(groups 1 "$A1" 1 "$R1")"
expect PASS "both tuples established, unchanged" "$(repo both2 "$both")" "$both" \
  "$(groups 1 "$A1" 1 "$R1")"
expect FAIL "edited RF case hash" "$(repo rfedit "$both")" \
  "$(manifest "$(tuple 1 1.0 "$A1")" "$(rf_tuple 1 "$R2")")" "$(groups 1 "$A1" 1 "$R2")"
expect FAIL "RF output changed under the same tuple" "$(repo rfdrift "$both")" "$both" \
  "$(groups 1 "$A1" 1 "$R2")"
expect FAIL "embedded lowerer bump reusing the old RF tuple" "$(repo rfold "$both")" \
  "$(manifest "$(tuple 1 1.0 "$A1")" "$(tuple 2 1.0 "$A2")" "$(rf_tuple 1 "$R2")")" \
  "$(groups 2 "$A2" 1 "$R2")"
expect PASS "embedded lowerer bump with new tuples" "$(repo rfbump "$both")" \
  "$(manifest "$(tuple 1 1.0 "$A1")" "$(tuple 2 1.0 "$A2")" "$(rf_tuple 1 "$R1")" "$(rf_tuple 2 "$R2")")" \
  "$(groups 2 "$A2" 2 "$R2")"
expect FAIL "StateMachine tuple edited beside an RF tuple" "$(repo smedit "$both")" \
  "$(manifest "$(tuple 1 1.0 "$A2")" "$(rf_tuple 1 "$R1")")" "$(groups 1 "$A2" 1 "$R1")"

# Feature 503 (SC-010, T050): an approval tuple; a co-supplied flow is part of its key.
ap_t='{"core_release":{"commit":"c1.0","version":"1.0"},"lowerer_version":1,"model_kind":"approval","model_semantic_version":1'
co=',"co_supplied":[{"kind":"request_flow","lowerer_version":1,"model_semantic_version":1},{"kind":"state_machine","lowerer_version":1,"model_semantic_version":1}]'
ap_tuple() { # ap_tuple <with co_supplied: 1|0> <cases json>
  if [ "$1" = 1 ]; then printf '%s%s,"cases":%s}' "$ap_t" "$co" "$2"; else printf '%s,"cases":%s}' "$ap_t" "$2"; fi
}
ap_computed() { # ap_computed <with co_supplied: 1|0> <cases json>
  if [ "$1" = 1 ]; then printf '{"tuple":%s%s},"cases":%s}\n' "$ap_t" "$co" "$2"
  else printf '{"tuple":%s},"cases":%s}\n' "$ap_t" "$2"; fi
}
G='"generated":{"p":{"derived":{"approved":"sha256:d1"},"entity":{"hash":"sha256:x1","name":"PDecision"},"enum":{"hash":"sha256:c1","name":"PDecisionChoice"}}}'
P1="{\"ap\":{\"behavior_hash\":\"sha256:b1\",$G,\"models\":{\"p\":\"sha256:p1\"}}}"
P2="{\"ap\":{\"behavior_hash\":\"sha256:b1\",$G,\"models\":{\"p\":\"sha256:p2\"}}}"
apbase="$(manifest "$(ap_tuple 1 "$P1")")"
expect PASS "approval tuple, unchanged" "$(repo ap "$apbase")" "$apbase" "$(ap_computed 1 "$P1")"
expect FAIL "edited approval hash" "$(repo apedit "$apbase")" "$(manifest "$(ap_tuple 1 "$P2")")" \
  "$(ap_computed 1 "$P2")"
expect FAIL "approval lowering changed without a version bump" "$(repo apdrift "$apbase")" \
  "$apbase" "$(ap_computed 1 "$P2")"
expect FAIL "approval case recorded without its co-supplied versions" "$(repo apkey "$base")" \
  "$(manifest "$(tuple 1 1.0 "$A1")" "$(ap_tuple 0 "$P1")")" "$(ap_computed 1 "$P1")"

# Feature 505: explicit supersession (contract compat-supersession.md §6).
smkey() { # smkey <semantic> <lowerer>: the full key of a StateMachine tuple of Core 1.0
  printf '{"core_release":{"commit":"c1.0","version":"1.0"},"lowerer_version":%s,"model_kind":"state_machine","model_semantic_version":%s}' \
    "$2" "$1"
}
smt() { # smt <semantic> <lowerer> <cases json> [<supersedes json array>]
  local sup=""
  if [ -n "${4:-}" ]; then sup=",\"supersedes\":$4"; fi
  printf '{"core_release":{"commit":"c1.0","version":"1.0"},"lowerer_version":%s,"model_kind":"state_machine","model_semantic_version":%s%s,"cases":%s}' \
    "$2" "$1" "$sup" "$3"
}
smc() { # smc <semantic> <lowerer> <cases json>: computed StateMachine cases
  printf '{"tuple":{"core_release":{"commit":"c1.0","version":"1.0"},"lowerer_version":%s,"model_kind":"state_machine","model_semantic_version":%s},"cases":%s}\n' \
    "$2" "$1" "$3"
}
v1="$(manifest "$(smt 1 1 "$A1")")"
expect PASS "superseded by a higher semantic version" "$(repo sup "$v1")" \
  "$(manifest "$(smt 2 1 "$A2" "[$(smkey 1 1)]")")" "$(smc 2 1 "$A2")"
expect FAIL "removed without supersedes" "$(repo supnone "$v1")" \
  "$(manifest "$(smt 2 1 "$A2")")" "$(smc 2 1 "$A2")"
v1ab="$(manifest "$(smt 1 1 "$AB")")"
expect FAIL "supersession that loses a case" "$(repo suplost "$v1ab")" \
  "$(manifest "$(smt 2 1 "$A2" "[$(smkey 1 1)]")")" "$(smc 2 1 "$A2")"
rfsup="{\"core_release\":{\"commit\":\"c1.0\",\"version\":\"1.0\"},\"embedded\":[{\"kind\":\"state_machine\",\"lowerer_version\":1,\"model_semantic_version\":2}],\"lowerer_version\":1,\"model_kind\":\"request_flow\",\"model_semantic_version\":2"
expect FAIL "superseded by another kind" "$(repo supkind "$v1")" \
  "$(manifest "$rfsup,\"supersedes\":[$(smkey 1 1)],\"cases\":$A2}")" \
  "{\"tuple\":$rfsup},\"cases\":$A2}"
v12="$(manifest "$(smt 1 2 "$A1")")"
expect FAIL "mixed version vector" "$(repo supmixed "$v12")" \
  "$(manifest "$(smt 2 1 "$A2" "[$(smkey 1 2)]")")" "$(smc 2 1 "$A2")"
expect FAIL "supersedes names a tuple still present (equal vector)" "$(repo supsame "$v1")" \
  "$(manifest "$(smt 1 1 "$A1" "[$(smkey 1 1)]")")" "$(smc 1 1 "$A1")"
expect FAIL "supersedes names a tuple that was never established" "$(repo supnever "$v1")" \
  "$(manifest "$(smt 2 1 "$A2" "[$(smkey 0 1),$(smkey 1 1)]")")" "$(smc 2 1 "$A2")"
co2=',"co_supplied":[{"kind":"request_flow","lowerer_version":1,"model_semantic_version":2},{"kind":"state_machine","lowerer_version":1,"model_semantic_version":2}]'
apkey1="$ap_t$co}"
expect PASS "approval superseded by higher co-supplied versions" "$(repo supap "$apbase")" \
  "$(manifest "$ap_t$co2,\"supersedes\":[$apkey1],\"cases\":$P2}")" \
  "{\"tuple\":$ap_t$co2},\"cases\":$P2}"
# After the merge, `main` holds the replacement with its accepted `supersedes` history.
v2main="$(manifest "$(smt 2 1 "$A2" "[$(smkey 1 1)]")")"
expect PASS "accepted supersession history after the merge" "$(repo suphist "$v2main")" \
  "$v2main" "$(smc 2 1 "$A2")"
expect FAIL "accepted supersession history edited" "$(repo suphistedit "$v2main")" \
  "$(manifest "$(smt 2 1 "$A2")")" "$(smc 2 1 "$A2")"
# An unbuilt tuple cannot take over cases: here it would hide that the built v3 lost case b.
expect FAIL "superseded by a tuple that is not built" "$(repo supunbuilt "$v1ab")" \
  "$(manifest "$(smt 2 1 "$AB" "[$(smkey 1 1)]")" "$(smt 3 1 "$A2")")" "$(smc 3 1 "$A2")"

# --write --supersede records the replacement with `supersedes` and removes the old tuple.
wr="$(repo supwrite "$v1")"
printf '%s' "$(smc 2 1 "$A2")" >"$tmp/computed.json"
if python3 "$here/check-models-compat.py" --root "$wr" --computed "$tmp/computed.json" \
    --write --supersede >/dev/null 2>"$tmp/err" \
  && python3 "$here/check-models-compat.py" --root "$wr" --computed "$tmp/computed.json" \
    >/dev/null 2>>"$tmp/err" \
  && python3 - "$wr/models/fixtures/compat.json" <<'PY' 2>>"$tmp/err"
import json, sys
tuples = json.load(open(sys.argv[1]))["tuples"]
assert [t["model_semantic_version"] for t in tuples] == [2], tuples
assert tuples[0]["supersedes"][0]["model_semantic_version"] == 1, tuples
PY
then :; else
  echo "test_models_compat: --write --supersede: $(cat "$tmp/err")" >&2
  failures=$((failures + 1))
fi

# Two version bumps on one branch before the merge (v1 -> v2 -> v3): the writer keeps the
# established predecessor v1, drops the branch-local v2, and the gate passes.
wr2="$(repo supwrite2 "$v1")"
printf '%s' "$(smc 2 1 "$A2")" >"$tmp/computed2.json"
printf '%s' "$(smc 3 1 "$A1")" >"$tmp/computed3.json"
if python3 "$here/check-models-compat.py" --root "$wr2" --computed "$tmp/computed2.json" \
    --write --supersede >/dev/null 2>"$tmp/err" \
  && python3 "$here/check-models-compat.py" --root "$wr2" --computed "$tmp/computed3.json" \
    --write --supersede >/dev/null 2>>"$tmp/err" \
  && python3 "$here/check-models-compat.py" --root "$wr2" --computed "$tmp/computed3.json" \
    >/dev/null 2>>"$tmp/err" \
  && python3 - "$wr2/models/fixtures/compat.json" <<'PY' 2>>"$tmp/err"
import json, sys
tuples = json.load(open(sys.argv[1]))["tuples"]
assert [t["model_semantic_version"] for t in tuples] == [3], tuples
assert [s["model_semantic_version"] for s in tuples[0]["supersedes"]] == [1], tuples
PY
then :; else
  echo "test_models_compat: two bumps before the merge: $(cat "$tmp/err")" >&2
  failures=$((failures + 1))
fi

# No merge base: the gate fails, it never passes silently.
nobase="$tmp/nobase"
mkdir -p "$nobase/models/fixtures"
git -C "$nobase" init -q -b trunk
# Feature 507 (research R13): language levels coexist. A new semantic-version tuple of the same
# kind is added beside the established one; nothing is superseded, and both must reproduce.
sm_level() { # sm_level <semantic version> <cases json>
  printf '{"core_release":{"commit":"c1.0","version":"1.0"},"lowerer_version":1,"model_kind":"state_machine","model_semantic_version":%s,"cases":%s}' "$1" "$2"
}
levels() { # levels <level-1 cases> <level-3 cases>
  printf '{"groups":[{"tuple":{"core_release":{"commit":"c1.0","version":"1.0"},"lowerer_version":1,"model_kind":"state_machine","model_semantic_version":1},"cases":%s},{"tuple":{"core_release":{"commit":"c1.0","version":"1.0"},"lowerer_version":1,"model_kind":"state_machine","model_semantic_version":3},"cases":%s}]}\n' "$1" "$2"
}
B3='{"b":{"behavior_hash":"sha256:b3","models":{"n":"sha256:n3"}}}'
B4='{"b":{"behavior_hash":"sha256:b4","models":{"n":"sha256:n4"}}}'
expect PASS "a new language level beside the established one" "$(repo level "$base")" \
  "$(manifest "$(sm_level 1 "$A1")" "$(sm_level 3 "$B3")")" "$(levels "$A1" "$B3")"
expect FAIL "the established level changed beside a new level" "$(repo leveldrift "$base")" \
  "$(manifest "$(sm_level 1 "$A1")" "$(sm_level 3 "$B3")")" "$(levels "$A2" "$B3")"
expect FAIL "the new level's case drifted" "$(repo level3drift "$base")" \
  "$(manifest "$(sm_level 1 "$A1")" "$(sm_level 3 "$B3")")" "$(levels "$A1" "$B4")"

printf '{"commit":"c1.0","version":"1.0"}\n' >"$nobase/core-release.json"
expect FAIL "no merge base" "$nobase" "$base" "$(computed 1 "$A1")"

if [ "$failures" -ne 0 ]; then
  echo "test_models_compat: $failures failure(s)" >&2
  exit 1
fi
echo "test_models_compat: OK"
