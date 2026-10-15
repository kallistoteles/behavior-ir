# Validation guide: Core Release 0.13.0 alignment

Run from `~/repos/behaviour-ir-504` inside `nix develop`, with
`CARGO_TARGET_DIR=~/repos/behaviour-ir/target` (research R6).

1. Pin and assets: `scripts/check-core-pin.sh` → `OK (v0.13.0 1509497…)`;
   `scripts/fetch-core.sh` downloads and verifies both assets. Altering one sha256 in a copy of
   `core-release.json` (via `--root`) makes the fetch fail.
2. Versions: `pytest python/tests/test_core_alignment.py python/tests/test_versions.py` →
   pass; values per [contracts/versions.md](contracts/versions.md).
3. US2 regression: `cargo test -p behavior-models --test query_command_profile` and
   `pytest python/tests/test_query_command_profile.py` → allowed+commit, refused, altered
   evidence refused.
4. Existing contract: `python3 scripts/check-models-compat.py` passes; full
   `scripts/gates.sh` passes. Every changed expected output is listed in
   `implementation-log.md` with its Core reason (target: fresh record version/hashes only).
5. Release: `scripts/release-check.sh` passes (clean install outside the checkout reports
   core 0.13.0 and the exact commit).

Expected: no edits in the Core repository; `git diff` touches only the files in plan.md.
