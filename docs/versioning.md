# Versioning

A Behavior release has one version number, but it carries several independent versions. They
change for different reasons. Core/binding versions are reported through `behavior.versions()`,
`behavior engine-info` and `release-manifest.json`. Safety analysis/evidence additionally reports
its checker, statement and pinned proof-toolchain identities; those are separate from Core
engine/verifier versions.

| Kind | Example | What it identifies | Where it lives |
|---|---|---|---|
| **Release** | `0.14.0` | this repository's release: the Python package and everything it bundles | `[workspace.package] version` in `Cargo.toml`, the single source |
| **Core** | `0.15.0` | the exact Core Release the package is built against and bundles (feature 011) | `core-release.json`; `behavior.versions()["core"]` reports the full immutable commit |
| **Binding** | `python 0.14.0` | the installable package of the Python binding | the release version in Python's form (PEP 440); the package requires *exactly* the core in `core-release.json` (checked at import) |
| **Formats and verifier** | accepted wire IR `0.4` … `0.8`, records `0.4` … `0.8`, verifier `0.10.0` | the document formats and the verification encoding | defined by the core: [its versioning](https://github.com/kallistoteles/behavior-ir-core/blob/v0.15.0/docs/versioning.md); reported by `behavior engine-info` |
| **Safety certification** | checker `3`, statement `2`, evidence/bundle `v2` | build-time checking and proof-data contracts; policy meanings remain v1 | Safety analysis/evidence; `crates/behavior-models/src/safety/` and `crates/behavior-models/proofs/toolchain.json` |

A module document, a record or a store document says which format version it is written in.
That is what a reader checks, not the release number.

## Release version policy (0.x)

While the release is below 1.0, the **minor** number marks anything a consumer must adapt to:

- **Minor bump** (`0.8.x` → `0.9.0`):
  - any change to behavior identities (item hashes, behavior versions);
  - a new wire IR, record or store document version, or a change to an existing one;
  - removing or changing a public API element (see `api/public-api.json`);
  - a verifier change that can alter outcomes, together with a `VERIFIER_VERSION` bump.
- **Patch bump** (`0.8.0` → `0.8.1`):
  - additive public API;
  - fixes that change no identity, format or verification outcome;
  - documentation and skills.

After 1.0, the same rules apply with **major** in place of minor.

**No backward compatibility before 1.0.** Until the first stable release, semantic correctness
takes precedence over compatibility: a minor release may change or remove any API, call pattern
or helper without a deprecation period, compatibility shim or fallback, and consumers migrate to
the new contract. What never changes by accident is determinism, canonical content identity
where semantically required, and evidence integrity; a change of semantic meaning is a
deliberate design decision, documented with its release.

Release 0.14.0 changes Safety certification outcomes and its evidence contract (feature 511),
so it follows the minor rule. Safety checker 3 and statement contract 2 use evidence/bundle v2.
This checker supports v2; historical v1 artifacts retain their original pinned verification
conditions and are never rewritten or implicitly reinterpreted. Reproducible historical checking
alone does not establish that an older implementation is sound. Safety policy semantics stay 1.
Core remains exactly 0.15.0 with verifier 0.10.0; Core behavior identities, runtime state/replay
and existing behavioral Model receipts are unchanged. Implementation and full acceptance checks
are tracked separately; version metadata does not claim that those checks have passed.

Release 0.13.1 is the first published Safety Models API: FieldOwnership v1 and the restricted
LifecycleDependency v1, with Safety checker/lowerer 2. This is additive relative to published
0.13.0, which contains no Safety API. Checker-1/2 development outcomes were not published in
0.13.0; their final recognition/diagnostic corrections do not change a released verifier outcome.
Core stays at 0.15.0 with verifier 0.10.0, and established Core identities, formats and Model
receipts remain unchanged. This therefore follows the patch rule above. Safety policy semantics
remain version 1; checker/source identities distinguish evidence and reject stale bundles.
See the [0.13.1 notes](../release/notes/0.13.1.md).

Release 0.13.0 bundles exact Core Release 0.15.0, commit
`6b93898a9375113f7a8946f23ab5535319b6f5d6` (adopted by feature 508), and verifier 0.10.0.
The minor bump from published 0.12.0 follows the existing policy: new decision records,
changed verifier outcomes and snapshot/backend contracts require consumer adaptation.
Additive Models and Skills alone would qualify for a patch; they are not the reason for this
minor bump. StateMachine, RequestFlow, Approval, WorkQueue and composition ship together;
see the [release notes](../release/notes/0.13.0.md).

Consumer adaptations include:

- **Complete parent snapshots (Core 017).** Fresh action evaluation establishes complete
  resulting-state validity and writes decision record 0.8 (`validation_profile`
  `behavior.complete_resulting_state.v1`, the complete parent universe archived). Direct
  `evaluate` and `evaluate_intent` therefore take a required `facts` argument whose `universe`
  lists every declared entity type, empty ones as `members: []`; nothing is inferred from the
  bound state. An incomplete or invalid parent snapshot is `INVALID_INPUT` with
  `INVALID_STATE_SNAPSHOT` (an invalid parent is no longer reported as `INVALID_STATE`). Stores
  supply the snapshot themselves. Historical records keep their versions and replay unchanged.
- **Verifier 0.10.0** includes 0.9.0's corrected invalid-child-state outcomes and fixes Approval
  query congruence (Core #35). The unchanged reference Approval's local uniqueness
  postconditions prove; Feature 506 C19 is no longer blocked. Other precision limits remain.
  Fresh authenticated verification needs policies selected for verifier 0.10.0; existing
  policies and historical authorizations are not upgraded implicitly.
- **Store creation (Core 019).** `Backend.create` must recover an equivalent original creation
  as an idempotent no-op and refuse a different one; the backend conformance suite has 45
  cases. Custom backends must implement this.
- **Command-profile query evidence (Core 023).** Actions that observe a query under the command
  profile now commit; query evidence in bundles is validated strictly.
- `accepted_wire_ir` reports `0.4` … `0.8`, what fresh admission actually accepts (Core 020);
  command-free Python DSL retains its established profile, while public `@command`/`emit`
  authoring selects the command profile. Existing reference identities remain unchanged.

See the pinned core documentation for
[invocation](https://github.com/kallistoteles/behavior-ir-core/blob/v0.15.0/docs/invocation.md),
[persistence](https://github.com/kallistoteles/behavior-ir-core/blob/v0.15.0/docs/persistence.md)
and [governance](https://github.com/kallistoteles/behavior-ir-core/blob/v0.15.0/docs/governance.md).

Release 0.12.0 aligns the package with Core Release 0.12.0 (ecosystem feature 500). Its minor
bump reflects current document formats, verifier outcomes and governance refusal behavior:

- `BehaviorModule.from_wire_json` admits current documents explicitly. The existing Python DSL
  retains its legacy profile, semantic identities and replay paths.
- Unified invocation handles reads and actions with typed bindings, explicit snapshots and
  host context. The package exposes canonical invocation records, candidates and replay.
- Wire IR 0.8 introduces command intents and decision record 0.7. Durable occurrences are
  streamed using exact history anchors; source diagnostics remain outside record identity.
- Current reads on Wire 0.8 produce `behavior.read_record.v2`. Historical v1 records retain
  their replay path. Verifier 0.8.0 corrects read-filter assumptions and entity-role validity;
  fresh verification outcomes can therefore differ from the older release.
- Store-v2 and trusted governance are explicit. Fresh writes to required-governance v1
  histories refuse with `TRUSTED_GOVERNANCE_UPGRADE_REQUIRED`, including migrations. Exporting
  validated state into a new v2 genesis preserves content without authenticating old history.
- The bundled CLI exposes `invoke`, `invoke-intent`, `invoke-replay` and `governance`. Signed
  proof and authorization generation use that CLI; Python commits independently supplied
  authorization context through the public core contract.

Core's historical `wire_ir` version-report array remains 0.1–0.7. The additional
`accepted_wire_ir` array reports decoder support through 0.8. `records`, `read_records`,
`command_stream` and `command_occurrence_domain` report the actual current formats/domains.
The ecosystem public API manifest lists all accepted wire formats. Command-free Python
modules retain their established profile; declaring commands selects the current profile.

See the pinned core documentation for
[invocation](https://github.com/kallistoteles/behavior-ir-core/blob/v0.12.0/docs/invocation.md),
[commands](https://github.com/kallistoteles/behavior-ir-core/blob/v0.12.0/docs/commands.md) and
[governance](https://github.com/kallistoteles/behavior-ir-core/blob/v0.12.0/docs/governance.md).

Release 0.10.4 makes the package reproducible across machines (SC-011): maturin's generated
Rust SBOM recorded the absolute build directory, so the wheel built in CI and one built locally
from the same commit differed in that file alone. The SBOM is off until a path-independent one is
produced, and the release check refuses any wheel member that embeds a build directory. Still
bundles Core Release 0.10.2.

Release 0.10.3 is the first published release of the ecosystem as its own repository, bundling
Core Release 0.10.2. The tag `v0.10.2` exists but was never released: its release check failed
on a test that inherited the workflow's required-check list from the environment. Under the tag
rule (a tag is never moved) the fixed test ships as 0.10.3. From here the ecosystem's release
version and the core's differ; the package names its core exactly.

Release 0.10.2 is a patch release: the repository split (feature 011). Behavior Core moved to
behavior-ir-core and is consumed as Core Release 0.10.2, pinned by commit, through its public
`behavior-engine` crate. The package bundles the core's `behavior` CLI and reports the core it
contains. No identity, format or verification outcome changes. The ecosystem's release version
follows the core's for now; the two may diverge later, and the package always names its core
exactly.

Release 0.10.0 was a minor bump for three reasons:

- **Wire IR 0.7:** a module's `reads` section (declared reads, a new behavior item kind) and the
  read document (an ad-hoc read). A module with declared reads has a new behavior version, never a
  new schema; a module without them keeps its bytes and identity.
- **A new document kind:** the read record `behavior.read_record.v1`, with its identity
  `read:sha256:…`.
- **`VERIFIER_VERSION` 0.6.0**, for the `evaluation_error` checks of declared reads.

Every existing module, decision record, store document, hash vector and attestation format keeps
its bytes.

Release 0.9.0 was a minor bump for three reasons:

- **Exact store-schema binding** replaces the touched-types comparison. A module that differs from
  the store only in an entity type its action does not touch used to evaluate and now gets
  `SCHEMA_MISMATCH`, and the old code `ENTITY_DECLARATION_MISMATCH` is no longer reported for it.
- **A new document kind:** the migration IR 0.1, plus optional fields that existing documents
  never carry: `Head.schema`, the record `kind` and `migration` fields, and
  `EvidencePolicy.migration`.
- **`VERIFIER_VERSION` 0.5.0**, for migration verification and the narrowing obligation.

Every existing module, record, genesis, store head (of a store that never migrated), evidence
policy and attestation format keeps its bytes.

## Compatibility promise

- Documents written by any release of a minor line (modules, records, stores, attestations) are
  read and replayed by every later release of the same line with identical results.
- A release that cannot read a document refuses it explicitly: the wire, record and store
  version gates report the unsupported version. It never produces different results
  silently.
- An existing document format never changes its bytes. New forms get a new version; documents
  without them keep their old version and bytes.

## Releases

A release is the annotated tag `v<version>` on a commit that passed `scripts/release-check.sh`,
together with the artifacts `scripts/release.sh` built into `dist/v<version>/`. Tags are never
moved; a fix is a new patch release.

### Safety certification capacity records

Behavior 0.14.0 retains Core 0.15.0, verifier 0.10.0 and policy semantics v1, with checker 3,
statement 2 and evidence/bundle v2. The exact candidate artifact and original measurement profile
are bound separately. All 20 frozen requests passed three complete cold certification/checking
pairs each; this is finite x86_64-linux capacity evidence, not universal completeness or an
ARM latency promise. See [recorded costs, source/tool identities and limits](../specs/511-safety-certification-scale/scalability-results.md).
Historical records retain their original formats and pinned verification conditions. A new
delivered artifact cannot silently inherit a differently hashed candidate's performance evidence.
