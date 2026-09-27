# Proposal: Persistence contract (state, transition, commit)

**Status**: implemented by feature 005 (`specs/005-persistence-contract/`, `docs/persistence.md`).
Deviations from this proposal:
- The proposal's `StateStore` is split into engine-owned `Store` semantics over a seven-method host
  `Backend`.
- The state identity binds entity content only (no revisions), via a MuHash3072 multiset hash.
- Concurrency is whole-state; entity-level concurrency is prepared but not implemented.
- The entity universe is fixed after the genesis.
- Evidence requirements come from a content-addressed evidence policy per store.
**Recorded**: 2026-09-27

Principle:

> Behavior Engine defines the canonical state-transition and persistence contract; hosts choose
> how that contract is persisted.
>
> Motorn definierar vad som måste bevaras för att beteendet ska kunna reproduceras. Den
> dikterar inte hur det lagras.

## Relation to what exists today (after feature 002)

- ΔS entries already address cells as `{entity, id, field}` (record_version 0.2), independent of
  parameter names ("Identity is semantic; parameter names are only bindings").
- The decision record already holds state, input, context, trace, and changes; `replay`
  re-evaluates it.
- The commit authorization (`behavior.authorization.v1`) already cites `transition_hash`,
  `policy_hash`, the attestation hash, and waivers used: the evidence half of a `CommitBundle`.
- Missing: entity revisions, state identity (`StateId = hash(canonical state)`), a parent state
  on each transition, the commit interface, and `STATE_CONFLICT`.
- Related open risks from `specs/002-smt-verification/checklists/implementation-review.md`:
  unsigned attestations and a trusted verification cache; a store that holds evidence should
  settle who produces and trusts it.

## Original proposal (verbatim)

Ja — då tror jag vi ska göra en tydlig skillnad mellan persistensmodell och
persistensimplementation.

Behavior-motorn kan definiera exakt vilka data som måste kunna sparas för att systemets semantik
ska bevaras, men den ska inte bestämma om implementationen använder Postgres, SQLite, Kafka,
filer eller något annat.

Jag skulle alltså lägga till ett lager:

```text
Behavior IR
    │
    ├── Entity / information model
    ├── Actions / transitions
    └── Invariants
             │
             ▼
      Persistence Contract
             │
             ▼
       Host implementation
             │
      ┌──────┼──────┐
      ▼      ▼      ▼
   Postgres SQLite Event store
```

### Vad motorn behöver modellera

Jag tror att fyra koncept räcker väldigt långt.

#### 1. Entity identity + revision

```text
EntityVersion {
    entity_type
    entity_id

    revision
    parent_revision?

    value
    value_hash
}
```

Exempel:

```text
Account #123

revision 17
balance = 100

        ↓ transfer

revision 18
balance = 80
```

Motorn behöver inte veta hur revision 17 är lagrad. Den behöver bara kunna referera till den
exakt.

#### 2. TransitionRecord

Det här tycker jag ska vara den centrala historikposten:

```text
TransitionRecord {
    id

    behavior_hash
    action_id

    state_before
    input
    context

    delta

    state_after

    trace

    verification_refs
    waiver_refs
    execution_policy_hash

    timestamp
}
```

Det beskriver:

> Med denna exakta behavior-version, på detta state, med dessa inputs och denna context,
> producerades denna förändring.

Det är egentligen systemets commit object.

#### 3. ΔState som förstaklassobjekt

Vi har redan:

```text
T(S, I, C) → ΔS
```

Så lagringsmodellen bör inte förlora det.

Exempel:

```text
Delta {
    Account#123.balance:
        100 → 80

    Account#456.balance:
        40 → 60
}
```

Det gör audit, replay och diff väldigt naturligt.

Och eftersom effects adresseras genom `EntityIdentity + FieldIdentity` snarare än
Python-parameternamn är förändringen oberoende av implementationen.

#### 4. State version / state root

Sedan behöver vi någon representation av:

> Vilket exakt systemstate gäller här?

Konceptuellt:

```text
StateVersion {
    parent_state?
    transition?
    entities
}
```

med:

```text
StateId = hash(canonical state)
```

Det betyder inte att databasen måste spara hela världen varje gång. Implementation kan använda
full snapshots, delta log, MVCC rows, event sourcing eller Merkle tree — men den logiska modellen
är densamma.

### Ett mycket litet persistence-interface

Rust-kärnan skulle exempelvis kunna definiera något konceptuellt i stil med:

```rust
trait StateStore {
    fn current(&self) -> StateRef;

    fn load(
        &self,
        entity: EntityId,
        at: StateRef
    ) -> EntityState;

    fn commit(
        &mut self,
        expected_parent: StateRef,
        transition: TransitionRecord
    ) -> Result<StateRef, CommitError>;

    fn transitions(
        &self,
        from: StateRef,
        to: StateRef
    ) -> Vec<TransitionRecord>;
}
```

Men själva motorn implementerar inte databasen. En användare skulle senare kunna skriva
`PostgresStateStore`, `SQLiteStateStore`, `InMemoryStateStore`, `FileStateStore` utan att ändra
Behavior IR.

### Det viktiga kontraktet

Motorn borde kunna lämna ifrån sig ungefär:

```text
CommitBundle {
    parent_state
    behavior_hash
    action
    delta
    trace
    verification_evidence
    policy_evidence
}
```

och säga till hosten:

> Spara detta atomiskt och returnera identiteten på det nya state som skapades.

### Snapshots är bara optimering

```text
S0 ↓ Δ1 → S1 ↓ Δ2 → S2 ↓ … ↓ Δ100000 → S100000
```

Implementation kan skapa `Snapshot S90000 + Δ90001 … Δ100000`, men snapshot är bara cache.
Transitionhistoriken är den semantiska sanningen — man kan alltid verifiera snapshoten genom
replay.

### Optimistic concurrency

Motorn evaluerar `State S42 → ΔS`. Hosten committar med `commit(expected_parent = S42, delta = ΔS)`.
Om databasen under tiden gått vidare till `S43` blir resultatet `STATE_CONFLICT` i stället för att
applicera ett beslut som evaluerades mot gammalt state. Nästan gratis när state har
identitet/version.

### Replay kräver ingen databasstandard

```text
state = initial_state
for transition in history:
    state = apply(state, transition.delta)
    assert hash(state) == transition.state_after
```

Eller starkare:

```text
result = evaluate(exact_behavior_version, state, recorded_input, recorded_context)
assert result.delta == recorded_delta
```

Då verifierar man inte bara datahistoriken utan själva determinismen hos behavior engine.

### Arkitektur

Inte "Behavior Engine inkluderar persistence", utan:

```text
                      Behavior IR
                           │
                           ▼
                    Runtime evaluator
                           │
                           ▼
                        ΔState
                           │
                           ▼
                     Commit Bundle
                    ┌──────┴──────┐
                    │ Persistence │
                    │   contract  │
                    └──────┬──────┘
                    implemented by host
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
      PostgreSQL        SQLite          Event store
```

Python-bindningen kan då börja väldigt enkelt:

```python
engine = Engine(model)
store = SqliteStore("app.db")

result = engine.evaluate(state=store.current(), action=transfer, input=...)
store.commit(result)
```

men `SqliteStore` är inte en del av Behavior-semantiken.

### Nästa steg

Designa ett minimalt State / Transition / Commit data model innan någon konkret databasadapter
byggs. Det kommer sannolikt påverka flera andra delar av motorn på ett bra sätt.
