# MCP conventions

Run `effectcraft-cli mcp` for a headless session, or `effectcraft-cli mcp --bridge 9877`
to use the desktop app's control channel. Existing ports are unchanged.

The server provides the same core conventions as FilmCraft #28:

| Tool | Arguments | Result |
| --- | --- | --- |
| `command_list` | `filter?`, `enabled_only?`, `schemas?` | Command catalog |
| `command_run` | `id`, `params?` | Engine command result |
| `command_batch` | `steps: [{id, params?}]`, `stop_on_error?` | `completed`, `failed`, per-step `results` |
| `doc_inspect` | none | Project overview and active composition |
| `render_preview` | `comp?`, `time?`, `max_side?`, `transparent?` | PNG and frame information |

`command_batch` stops at the first error by default; false continues. Each edit has its
own undo step. The existing `batch` tool retains its atomic undo grouping and result references.
Existing tools, including `list_commands`, `execute_command`, and `batch`, remain listed
because existing workflows use them; there are no hidden aliases. See [agents.md](agents.md)
for the full catalog. Each tool has a title and read-only, destructive, idempotent and
open-world hints. Tools with an optional output path are conservatively annotated as file writers.

Unknown top-level tool argument keys return JSON-RPC `-32602` naming the key and accepted
arguments. EffectCraft already rejects unknown engine command parameters; that behaviour is
preserved. Tool failures (including escaped panics) return `isError: true`. Malformed JSON
returns `-32700` with a null id and the session keeps serving. The MCP backend owns its session
without a mutex; the engine's render-job mutexes already recover poisoned locks.

Resources `effectcraft://document` and `effectcraft://commands` return JSON matching
`doc_inspect` and `command_list`. Clients declaring MCP 2026-07-28 in per-request `_meta`
receive `resultType: complete` and list/read cache hints; document reads are never cached.

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"command_run","arguments":{"id":"comp.new","params":{"width":640,"height":360}}}}
{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"effectcraft://document"}}
```
