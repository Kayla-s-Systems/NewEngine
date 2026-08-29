# newengine-engine-runtime

Reusable runtime composition layer for standalone GameFirst runtime apps.

This crate owns systems that are engine runtime responsibilities, not application responsibilities:

- scene bridge and runtime scene commands;
- gameplay components/schedules used by runtime profiles;
- render controller that talks only to `newengine-render-api`;
- material/texture residency orchestration above the render backend.

It deliberately does **not** own focused leaf capabilities that can live behind narrow contracts:

- world-audio lifecycle lives in `newengine-audio-world-runtime`; this crate consumes `newengine-audio-client` and `newengine-audio-world-api` only;
- visual asset preview lives in `newengine-asset-preview-runtime`;
- viewport synchronization lives in `newengine-viewport-bridge`;
- semantic input policy/state lives in `newengine-input-systems-runtime`, with capture messages in `newengine-input-capture-api`;
- editor viewport state, transform history, camera projection policy and gizmo planning live in `newengine-editor-viewport-runtime`;
- authored placement identity/dirty/clone/replica DTOs live in `newengine-world-authoring-api`.

For editor viewport integration this crate retains only thin composition adapters:

- apply editor camera projection to the engine view frame;
- admit/remove gizmo render entities through `SceneBridge` material/primitive registries;
- rebuild physics-facing authored-placement replicas after editor scale changes.

Those adapters may know engine Scene/physics implementation. `newengine-editor-viewport-runtime` may not depend back on this crate.

Standalone games depend on this crate through product profiles such as `newengine-game-ready-profile`. They must not call a native graphics API, create pipelines, upload textures, build shadow passes or assemble postFX directly.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-engine-runtime`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** runtime composition plus narrow adapters for extracted leaf capabilities.

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, clients, and explicit composition adapters.
- Do not move extracted leaf implementations back into this crate to avoid defining a contract.

<!-- NORTHSTAR-DIR-README:END -->

## Extracted bounded capabilities

The engine runtime is a composition/orchestration layer, not a service supermarket. Consumers should depend on the smallest owner that provides the capability they need.

- `newengine-audio-client` / `newengine-audio-world-api` — audio transport and transient contracts.
- `newengine-asset-preview-runtime` — provider-backed visual asset preview.
- `newengine-model-client` / `newengine-material-client` — provider-neutral asset clients.
- `newengine-viewport-bridge` — UI/render viewport synchronization.
- `newengine-input-systems-runtime` — semantic input systems and capture enforcement.
- `newengine-editor-viewport-runtime` — editor viewport controller/policy.
- `newengine-world-authoring-api` — authored world placement contract.
