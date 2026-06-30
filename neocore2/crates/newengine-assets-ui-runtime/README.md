# newengine-assets-ui-runtime

Runtime-hosted semantic compiler for `.neui` UI dictionaries.

Boundary:

```text
engine.assets / VFS          -> bytes
NEF8/ListFile validation     -> envelope/body integrity
engine.assets.ui             -> XMLcentral UI semantics + compile response
engine.ui                    -> live mounted runtime UI
```

Consumers issue request/response calls to `engine.assets.ui`; they do not parse `.neui`, NEF8, deflate, XMLcentral, or VFS details.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-assets-ui-runtime`

**Role:** Static provider-local assets used by this crate/plugin/module.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
