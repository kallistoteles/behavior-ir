# Contract: Migration Wire Document (migration IR 0.1)

```json
{
  "migration_ir": "0.1",
  "name": "v1_to_v2",
  "source": "sha256:<SchemaHash A>",
  "target": "sha256:<SchemaHash B>",
  "constants": [{"name": "unknown", "type": {"t": "string"}, "value": "n/a", "loc": {...}}],
  "requirements": [{"name": "every_order_has_region", "loc": {...},
                    "body": {"op": "all", "args": [{"op": "select", "entity": "Order"}], "param": "o",
                             "body": {"op": "is_some", "args": [{"op": "field", "param": "o", "field": "region"}]}}}],
  "transforms": [{"entity": "Order", "loc": {...},
                  "fields": {"region": {"op": "strict_unwrap", "args": [{"op": "field", "param": "old", "field": "region"}]}},
                  "drops": []}],
  "retire": ["AuditNote"]
}
```

- **Expressions** use module wire IR 0.6 syntax. The transform scope is the parameter `old`. A
  named type (enum, nominal, exact) and the nominal of `wrap`/`rescale` may carry
  `"side": "target"` (the default is `"source"`).
- **New operators:**
  - `strict_unwrap` (one argument);
  - `enum_map`, `{op, args: [x], to: <target enum type>, mapping: [[src, tgt], …]}`, which must
    be total (an absent option stays absent);
  - `strict_enum_map`, the same but possibly partial, which makes a narrowing site.
- **Resolved form.** Admission returns the resolved document: every changed type has a
  transform (also one the author did not mention), every target field is explicit (with
  `{"copy": f}` for automatic copies), and drops are listed. Admitting it again gives the same
  migration. A transform's `loc` is optional.
- **Identity.** The semantic hash of the resolved migration under tag `behavior.migration.v1`:
  source and target SchemaHash, constants, requirements (name and expression hash), transforms
  (fields in target declaration order as copy or expression hash, sorted drops) and retired
  types. `name` and locations are excluded, so spelling an automatic copy explicitly is the same
  migration.
- **Schema.** `schema/migration-ir-0.1.schema.json`.
