# Implementation Plan: Current Core Release alignment

**Branch**: `500-core-release-alignment` | **Date**: 2026-10-06 | **Spec**: [spec.md](spec.md)

## Summary

Publish validated core0.12.0, align canonical dependency/assets, preserve legacy Python
authoring and add checked wire import, invocation/replay and durable command history access.
Native v2 genesis/context/evidence paths preserve the independent-context commit requirement.
Trusted proof generation/authorization remains accessible through the packaged core CLI.
Prepared migration store operations remain public Rust APIs; new Python command/governance
DSLs are separate features.

## Technical Context

- Pinned Rust, Python3.13, PyO3 0.26; existing serde and behavior-engine facade only.
- Existing Backend protocol, opaque core documents and native atomic CAS.
- pytest, mypy, fmt/clippy, canonical gates and clean-install release checks under Nix.
- Existing CPython abi3 manylinux x86_64 package; no new evaluator or external effects.
- Raw JSON strings reach core checked decoders; diagnostics stay outside record identities.

## Constitution Check

| Principle | Design evidence |
| --- | --- |
| Deterministic core / validated input | Delegate every operation to behavior-engine. |
| Test-first | Observe new identity, invocation, commands and refusal tests fail first. |
| Reproducibility | Release fixtures and native replay; retain legacy builder profile. |
| State / auditability | Invocation records and exact history; no implicit commit. |
| Simplicity | Existing extension, documents and Backend; no new dependencies. |
| Packaging | Published exact git revision; canonical scripts without overrides. |
| Gates | Full gates, exact-revision consumer and clean install before delivery. |

Pre-research and post-design: PASS. No constitution deviation.

## Project Structure

- `crates/behavior-py/src/lib.rs`: native document operations and detached diagnostics.
- `python/behavior/`: module import, invocation result wrappers, store helpers and stubs.
- `python/tests/test_core_alignment.py`, `test_invocation.py`, `test_command_documents.py`:
  exact release, fixture parity, duplicate keys, refusals, replay and history ordering.
- `core-release.json`, Cargo metadata, public API manifest and release documentation.
- Feature artifacts in `specs/500-core-release-alignment/` record requirements and evidence.

## Delivery

Core release checks and the consumer cover b525da46d4282166418a90141f0c94fd099e9b47
before final artifact construction/tagging/publication. The ecosystem then uses published
assets only. Deliver a reviewable upgrade branch and verified changes to the user's checkout
without touching their Spec Kit edits. Ecosystem package publication is outside this upgrade.
