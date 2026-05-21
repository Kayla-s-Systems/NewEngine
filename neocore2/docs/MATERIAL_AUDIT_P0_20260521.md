# Material audit P0 — 2026-05-21

## Current material representations

- `newengine-materials::api::MaterialDescriptor` — renderer-agnostic numeric descriptor.
- `newengine-materials::api::MaterialTextureBindings` — authored/resolved texture dictionary selectors.
- `newengine-materials::api::AuthoredMaterialDescriptor` — source `.nemat` material descriptor entry.
- `newengine-materials::ResolvedMaterialGraph` — resolved gateway graph.
- `newengine-materials::RenderMaterialPacket` — renderer-facing packet.
- `newengine-render-api::provider_bridge::*Material*` — bridge DTO only; should not become authored material state.
- `newengine-engine-runtime::render_controller::material_bindings::*` — lowering view; should consume resolved packets/refs, not parse material files.
- UI draw materials are UI-domain primitives and must stay separate from world PBR material descriptors.

## Hard rules

- Renderer must not own authored material state.
- Scene bridge must not assemble material graphs procedurally.
- Material texture refs must be `.ytd@entry`; raw image paths and `.neytd@entry` are invalid in authored material graphs.
- AssetManager/VFS returns bytes and codec outputs; `engine.materials` interprets material descriptors.
- The only material resolve path is `engine.materials -> materials.api`.

## Follow-up cleanup targets

- `render_controller/material_bindings.rs`: keep as lowering layer only.
- `render_controller/resource_cache.rs`: keep texture residency/cache only; no authored material decisions.
- `scene_bridge/game_ready*`: move remaining inline material fields into `.nemat` libraries and drawable material slots.
- `newengine-model-runtime::material_binding`: treat OBJ/MTL conversion as importer compatibility only, not the authored path.
