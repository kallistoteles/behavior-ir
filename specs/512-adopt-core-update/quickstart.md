# Validation guide

Use `/home/kalle/repos/behavior-ir-512`. Existing 510/511 work remains in the original checkout.

## Preparation

Use the published 0.14.0 package in an isolated environment. Record its wheel checksum and `behavior.versions()`; do not use the original checkout's older developer venv. Source the pinned environment when solver/proof tools are needed.

```sh
python scripts/semantic-snapshot.py specs/512-adopt-core-update/evidence/baseline-semantic.json
python specs/512-adopt-core-update/probes/adoption_cases.py --require-supported
python specs/512-adopt-core-update/probes/growth.py --output baseline-growth.json
python -m pytest -q python/tests/test_pinned_identity_baseline.py
```

The removal support assertion is intentionally red on the old verifier; record the actual six failing cases alongside passing runtime controls. `growth.py` writes a baseline observation record, not candidate acceptance. The receipt-gate regression must pass after its narrow metadata correction.

## After a qualifying Core release is published

Select the published release/assets, update the exact pin and build the normal package without overrides. Freeze and use the baseline-derived growth profile before running the new candidate. Repeat the same public removal/growth controls and compare semantic snapshots separately from verification reports. Add current-Core compatibility tuples and regenerate/independently check fresh evidence.

```sh
scripts/check-core-pin.sh
scripts/gates.sh
scripts/conformance.sh 506 complete
scripts/conformance.sh 507 complete
scripts/release-check.sh
```

All required checks and the frozen performance criterion must pass. Record exact tested identities, remaining limitations and nonblocking findings. Do not tag or publish from these instructions.
