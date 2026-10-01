# Contract: Public API Manifest

`api/public-api.json` (canonical JSON, checked in):

```json
{
  "format": "behavior.public_api.v1",
  "release": "0.8.0",
  "python": {
    "module": "behavior",
    "names": {
      "entity": "decorator", "field": "function", "action": "decorator",
      "select": "function", "count": "function", "Store": "class",
      "versions": "function"
    }
  },
  "cli": {
    "admit": ["<wire>"], "version": ["<wire>"], "hashes": ["<wire>"],
    "eval": ["<wire>", "<request>"], "intent": ["…"], "replay": ["<wire>", "<record>"],
    "verify": ["<wire>", "--profile", "--cache"], "waiver-hash": ["<waiver>"],
    "sign-waiver": ["…"], "authorize": ["…"], "engine-info": []
  },
  "formats": {
    "wire_ir": {"0.6": "schema/wire-ir-0.6.schema.json"},
    "records": ["0.4", "0.5", "0.6"],
    "store_documents": ["behavior.commit_bundle.v1"]
  },
  "backend": {
    "required": ["genesis", "head", "create", "version_at", "version", "record",
                 "removed_at", "incoming_at", "keys_at", "commit"],
    "optional": ["used_at", "keys_by_field_at"]
  },
  "solver": {"name": "z3", "version": "4.16.0"}
}
```

(Excerpt. The real file lists every name in `behavior.__all__` and every CLI subcommand with its
flags.)

## Checks

| Check | Fails when |
|---|---|
| `python` | `set(behavior.__all__) != set(manifest.python.names)` |
| `cli` | the CLI's command tree (subcommands and flags) differs from `manifest.cli` |
| `backend` | the backend methods the store calls differ from `manifest.backend` |
| `skills` | a consumer skill references `behavior.X` or `behavior <cmd>` absent from the manifest |

A change to the public surface is a reviewed diff of this file. It is classified by the
versioning policy (additive = patch, removal/change = minor).
