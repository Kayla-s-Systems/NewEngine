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
metadata and provider-owned `.ysc` payloads.

Compatibility note: previous JSON frame/module DTOs remain available as adapters
while the engine migrates to `*_bytes_v1` methods. They must not reintroduce
language-specific branching.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-scripting-api`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
