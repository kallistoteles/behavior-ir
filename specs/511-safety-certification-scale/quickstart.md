# Feature 511 Validation Guide

Use the pinned installed candidate and Lean toolchain. No Core override, tag creation or
publication is part of this procedure. Choose fresh output directories; retain failed attempts.
See [evaluation contract](contracts/evaluation.md) for the complete acceptance requirements.

## Build and inspect the candidate

Run from the reviewed source snapshot in the repository's pinned development environment:

```bash
scripts/release-build.sh 0.14.0 /absolute/new-dist
(cd /absolute/new-dist && sha256sum -c SHA256SUMS)
python3.13 -m venv /absolute/new-consumer
/absolute/new-consumer/bin/python -m pip install --no-index /absolute/new-dist/behavior-0.14.0-cp313-abi3-manylinux_2_28_x86_64.whl

/absolute/new-consumer/bin/python specs/511-safety-certification-scale/experiments/inspect_candidate.py \
  --expected-wheel /absolute/new-dist/behavior-0.14.0-cp313-abi3-manylinux_2_28_x86_64.whl \
  --toolchain "$BEHAVIOR_SAFETY_TOOLCHAIN" --output /absolute/new-inspection
```

The inspection deliberately exhausts a tiny proof budget: it records the installed build identity
without issuing a certificate. It is not positive proof evidence. Run the existing delivery checks
before freezing a candidate, so package corrections precede the complete performance series:

```bash
scripts/release-check.sh /absolute/new-dist
scripts/conformance.sh 506 complete
scripts/conformance.sh 507 complete
```

`release-check.sh` runs `scripts/gates.sh` with `BEHAVIOR_FULL_PROPERTIES=1` and
`BEHAVIOR_RELEASE_TESTS=1`, then checks the installed artifact and reproducibility. For a gate-only
iteration, run `BEHAVIOR_FULL_PROPERTIES=1 scripts/gates.sh`; that does not replace the full release
check. Preserve all logs. Builds and gates run sequentially outside performance runs.
Use the existing supported environment variables; test-only compilation optimization does not alter
the delivered release, assertions, proof budgets or case inventory.

## Freeze exact inputs

```bash
python specs/511-safety-certification-scale/experiments/freeze_candidate.py \
  --source-root "$PWD" \
  --wheel /absolute/new-dist/behavior-0.14.0-cp313-abi3-manylinux_2_28_x86_64.whl \
  --inspection /absolute/new-inspection/inspection.json \
  --consumer-prefix /absolute/new-consumer \
  --profile specs/511-safety-certification-scale/evidence/evaluation-profile.json \
  --output /absolute/new-frozen-candidate.json
```

The source snapshot must contain all reviewed inputs in its index, including new proof and harness
files. The manifest records a precommit snapshot separately from HEAD and claims no published tag.
The original profile and hash sidecar fix all twenty cases, three repetitions, the x86_64-linux
reference conditions and separate 300 s budgets for complete certification and checking.
A256/A512 are minimum references, not a maximum supported size. Changing a relevant build input
requires a new manifest and complete reevaluation; acceptance conditions are never relaxed silently.

## Mandatory real scale entrypoint

```bash
BEHAVIOR_SAFETY_SCALE_PYTHON=/absolute/new-consumer/bin/python \
BEHAVIOR_SAFETY_SCALE_TOOLCHAIN="$BEHAVIOR_SAFETY_TOOLCHAIN" \
BEHAVIOR_SAFETY_SCALE_BUILD=/absolute/new-frozen-candidate.json \
BEHAVIOR_SAFETY_SCALE_OUTPUT=/absolute/new-final-runs \
python -m pytest -q specs/511-safety-certification-scale/experiments/tests/test_scale_acceptance.py

python specs/511-safety-certification-scale/experiments/collect_observations.py \
  /absolute/new-final-runs /absolute/new-corpus-assessment.json \
  --profile specs/511-safety-certification-scale/evidence/evaluation-profile.json \
  --build-manifest /absolute/new-frozen-candidate.json
```

Run all sixty pairs sequentially, without xdist or overlapping heavy jobs. Missing configuration,
cases, repetitions, proof obligations or reports fail acceptance. Fast gates explicitly exclude
this expensive file; fake-process harness tests cannot replace real certification.

Each pair uses separate installed producer/checker processes, new workspaces and the original
expected module/policies. Only the data bundle is transferred; producer-directory search is denied
during checking. This is a control for trusted tools, not a sandbox for hostile executables.
Loading a bundle never issues a certificate. The consumer reruns all required Lean checks and does
not trust a producer's previous approval.

Cold means installed pinned dependencies without previous request-result reuse. Installation and
downloads are separate; OS page cache is uncontrolled. Total role time includes staging, startup,
admission/extraction, checking, audits, fresh replay and semantic-result persistence. Stage limits,
CPU scope and sampled process-tree memory are recorded separately. Unavailable consumed-heartbeat
or recursion high-water counters are reported as unavailable, never as zero or as their ceilings.

## References and controls

```bash
export PATH="/absolute/new-consumer/bin:$PATH"
scripts/check-safety-proofs.sh --output /absolute/new-reference-proof-run
python specs/511-safety-certification-scale/experiments/bundle_controls.py \
  --positive /absolute/new-final-runs/A512-1/certification/bundle.json \
  --request /absolute/new-final-runs/A512-1/certification/request.json \
  --toolchain "$BEHAVIOR_SAFETY_TOOLCHAIN" --output /absolute/new-bundle-controls
```

Run these with the installed candidate selected on PATH. Reference checks cover the five existing
certificates and six Core/Safety guard comparisons. Actual Lean integrity controls, complete v1
regression coverage and reproducibility remain mandatory; their records are linked from
[acceptance evidence](acceptance-evidence.md). Core SMT outcomes, Safety certification, snapshot
validity and host authorization are separate claims. An over-budget completed proof can be valid
while capacity acceptance fails; exhaustion cannot certify or establish a counterexample.

## Recorded execution

The [frozen build](evidence/candidate-builds/final-01.json) used wheel SHA-256
`810566f34559851fb6e725179a7cb066e64fae3a3a25d3eb85e524855de3902b` and the original profile. The required pytest entrypoint
passed all 60 pairs in 9,476.77 s; the collector accepted all 60reports. See
[scaling results](scalability-results.md) for all 20 cases and the actual reference conditions.
These records apply to the reviewed precommit source snapshot and exact artifact, not a
published tag. Future delivered bytes must match or undergo the explicit reevaluation rule.
