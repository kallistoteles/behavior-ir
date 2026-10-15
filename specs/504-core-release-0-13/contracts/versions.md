# Contract: version reporting after 504

`behavior.versions()` equals `json.loads(behavior engine-info)` plus
`{"binding": {"python": "0.13.0"}}`, and in particular:

| Key | Value |
| --- | --- |
| `engine` | `0.13.0` |
| `verifier` | `0.9.0` |
| `records` | `0.4`, `0.5`, `0.6`, `0.7`, `0.8` |
| `accepted_wire_ir` | `0.4` … `0.8` (was `0.1` … `0.8`; Core 020) |
| `wire_ir` | `0.1` … `0.7` (unchanged, historical) |
| `read_records` | `behavior.read_record.v1`, `behavior.read_record.v2` (unchanged) |
| core revision | `15094972191aa0ae79705df0397c551a18745399` |

`behavior.__version__ == "0.13.0"`. `_check_versions` keeps its behavior. No Python API
element is added or removed (`api/public-api.json` changes only `release`).
