# newengine-scripting-api

Stable engine-facing contract for `engine.scripting`.

This crate is intentionally backend-agnostic. It exposes only the public gateway,
the generic `scripting.api` provider contract, the generic `scripting.backend`
capability root and serializable request/response envelopes.

Runtime invariant:

```text
engine.scripting receives opaque request/module bytes
provider interprets them privately
provider returns opaque response bytes
runtime validates the outer envelope/budgets/permissions
runtime applies only accepted engine-facing outputs
```

The base API must not declare known scripting languages, VM families or provider
kinds. Provider-specific language/schema/bytecode details belong inside provider
metadata and provider-owned `.ysc@entry` payloads.

Compatibility note: previous JSON frame/module DTOs remain available as adapters
while the engine migrates to `*_bytes_v1` methods. They must not reintroduce
language-specific branching.
