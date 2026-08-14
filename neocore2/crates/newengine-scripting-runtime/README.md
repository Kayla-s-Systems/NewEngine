# newengine-scripting-runtime

Runtime shell for the `engine.scripting` gateway.

This crate intentionally does not embed or name any scripting implementation. It
registers an engine-runtime baseline provider that accepts opaque `.ysc`
module bytes and opaque scripting request bytes, then returns an empty response
until a real provider overrides the `engine.scripting` route.

The primary path is:

```text
scripting.load_module_bytes_v1
scripting.invoke_bytes_v1
scripting.frame_bytes_v1
```

Deprecated JSON frame/module methods are kept as compatibility adapters so
existing engine code keeps working during migration. They do not interpret script
payloads and do not declare a language/VM whitelist.

Providers own interpretation. ECS/entity/scene/UI/audio mutation must happen via
validated engine-facing outputs and authoritative apply stages, never through
direct provider access to engine internals.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-scripting-runtime`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
