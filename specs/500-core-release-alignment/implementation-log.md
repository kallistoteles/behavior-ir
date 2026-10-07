# Implementation evidence

Work uses isolated core and ecosystem checkouts; original user work is untouched.

## Observed test-first evidence

Baseline native binding: ecosystem e5aefe994ca7c79bd6dbde02cadebda6ee36df0b, core0.10.2
aaded167d8a937e2357a0b01bf38f439c199a5e9. Tests were collected by the installed virtualenv
Python. An initial invocation using the system pytest executable failed during import and
was corrected; that collection failure is not counted as red evidence.

- Initial current-release/invocation/commands/refusal baseline: 86 collected; 85 failed, 1 passed. Log `/tmp/behavior-ecosystem-500-red.log`, SHA256 `ff587d629b1cdb8e4baeefbf4a5c1eb79510cbd2d93453292ca5ad94cd86b7d1`.
- Candidate/evidence/context helpers: 3 collected; 3 failed. Log `/tmp/behavior-ecosystem-500-governance-red.log`, SHA256 `f42b415c46f14c0de1bfea494e0e16292e05acbcdc90ed5e7cd603ca4821a332`.
- Raw legacy commit and duplicate keys: 2 collected; 2 failed. Log `/tmp/behavior-ecosystem-500-raw-commit-red.log`, SHA256 `6755c50c20c559283e18d3c8e3e338e3a1763b61e458b3204675d26639001a82`.
- Typed store request decode diagnostics: 1 collected; 1 failed. Log `/tmp/behavior-ecosystem-500-store-decode-red.log`, SHA256 `7d0106024edbb1f4a34f311c59bad74f3447d0476d85c307339e499a80feae33`.

Each implementation followed its observed requirement/API failure. Historical test-first
compliance is not established by this new evidence. Required final release results and exact tested revisions appear below as checks complete.

## Core validation

Core merge revision: b525da46d4282166418a90141f0c94fd099e9b47. GitHub required gates and
consumer are successful; canonical local release-check and exact pushed-revision consumer
completed successfully before final artifact construction.

- Canonical `scripts/release-check.sh`: passed. Log `/tmp/behavior-core-0.12.0-release-check.log`, SHA256 `7535d00ecda737692f8b9f697e3eea7450e05a5e3d982d8de9fee3db88294921`.
- Exact-revision `scripts/check-consumer.sh --rev b525da46d4282166418a90141f0c94fd099e9b47`: passed. Log `/tmp/behavior-core-0.12.0-exact-consumer.log`, SHA256 `9577d2e4a795e997dc6537c2ba7832cc858da48d7ffed1cb54ea964b7a04fa37`.
- Final `scripts/release.sh 0.12.0`: passed, including final checks and artifact construction. Log `/tmp/behavior-core-0.12.0-final-release.log`, SHA256 `c2b542a22038d402d83178323c581e42b6845bb3fdce9bf19b3bc287f29a8940`.

The annotated immutable tag `v0.12.0` was pushed. The canonical publication workflow
[37520928030](https://github.com/kallistoteles/behavior-ir-core/actions/runs/37520928030)
passed, including exact tagged-revision validation, final artifact construction and publication.
[Core Release 0.12.0](https://github.com/kallistoteles/behavior-ir-core/releases/tag/v0.12.0)
is published. Its downloaded manifest matches the locally validated final manifest exactly
and has SHA256 `d0713893cf7dde6bf2ec813cbbc136d01609470592aafc650d98a81a2b02c1f0`.

The canonical ecosystem pin derives from that published manifest. GitHub asset digests
agree with the manifest: CLI `02d1a3113ef00ba97be96782223ccb0ec768a0fb9c7ff4652ea71cba8ccfe925`,
conformance `d9f711c255ce9a21f427261b8e00468785a592f7836c9d3b6ce2acd57ce6a975`.

## Validation limits

Private compilation against the pushed unreleased core in a separate temporary checkout may
find wrapper compilation errors; it is not a required gate or release compatibility result.
Official ecosystem metadata/validation will use the published Core Release only.

## Ecosystem verification

The canonical checkout uses the published pin and canonical Cargo git resolution. A fresh
`maturin develop` rebuild followed the pin change; no private override or local artifact
source was present. `scripts/fetch-core.sh` downloaded and verified the actual release assets.

Focused tests collected and passed: **130 passed**, covering current-release identity,
invocation/command documents, store conformance, legacy stores/migrations and versions.
Log `/tmp/behavior-ecosystem-500-green.log`, SHA256
`b0e1b6cb0197f273a4344cf7bcccdd2d496699c768131731b90bf22ac1b5c07d`.
The observed private malformed-transport test failure was an overly specific error-message
expectation; its assertion now matches the native canonicalization diagnostic. Core's error
and ordered diagnostic behavior were preserved.

Formatting and Python types passed; independent native/Python API reviews found no blocking
mismatch. Full required gates and packaged release verification passed for implementation revision
`3fe06334d0898fd0629771a11f4aba7bc2612727` through canonical
`nix develop -c scripts/release-check.sh` (debug symbols/incremental caches disabled to fit
available temporary storage). No official check used an override or local release asset source.

- Full gates: **352 tests passed**, including collected models; formatting, clippy, mypy,
  determinism, ownership, terms, workflow and script checks passed.
- Packaged checks: clean hashed install without Rust, smoke and missing-solver paths,
  exact bundled CLI hash, version parity, byte identity with the repository and all skill
  examples passed. Public API/skills/equivalence: **48 tests passed**.
- Two rebuilds produced identical wheel bytes. Checked wheel SHA256:
  `9947987dd3d190679da882f3cee98ff21f0b9fbb4405acd325f4cdc4739c7606`.
- Full log `/tmp/behavior-ecosystem-0.12.0-release-check.log`, SHA256
  `82bb6b40967a1e819386a4b4a8e9bcdd4a969d77d5d837462b962886d1f164bc`.

An initial required public-surface check named nine unclassified feature-500 documents.
The ownership manifest now classifies new ecosystem feature numbers; no gate was bypassed.
Only the explicit ownership rule changed before the successful complete rerun.

## Delivery

The upgrade branch is published in [PR 14](https://github.com/kallistoteles/behavior-ir/pull/14).
The original `/home/kalle/repos/behaviour-ir` checkout is on that branch. All 14 pre-existing
modified/untracked files retain their recorded SHA256 hashes. The local virtualenv's old
interpreter no longer existed; the broken environment was preserved at
`/tmp/behavior-original-venv-before-0.12.0-20261007` and rebuilt with the pinned interpreter.
The installed local binding reports package/core 0.12.0 and the exact published core revision.

This evidence update changes documentation only. GitHub's required checks validate the
final delivered head through the normal PR workflow; merge follows successful checks.
Core 0.12.0 is published; this source upgrade does not publish an ecosystem package release.
