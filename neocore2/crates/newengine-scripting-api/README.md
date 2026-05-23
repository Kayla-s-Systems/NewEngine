# newengine-scripting-api

Stable engine-facing contract for `engine.scripting`.

This crate is data-only: it contains gateway constants, capability ids and serializable DTOs for script modules, frame inputs, command outputs, diagnostics and traces. Concrete runtimes such as Lua, visual scripting, WASM or future providers must live behind provider crates/plugins. The scripting API must not expose `&mut World`, native ECS storage, renderer handles, physics backend objects or AssetManager internals.

Runtime invariant:

```text
script provider receives DTO
script provider returns DTO commands/events/diagnostics
engine runtime validates and applies commands through authoritative domains
```
