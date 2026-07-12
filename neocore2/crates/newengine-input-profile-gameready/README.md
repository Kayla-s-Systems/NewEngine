# newengine-input-profile-gameready

GameReady FPS default input profile. This is product/profile content, not generic input binding infrastructure.

## Internal architecture

- `action.rs` — GameReady semantic action identifiers.
- `profile.rs` — complete and standalone-game profile assembly.
- `key_registry.rs` — canonical key catalog.
- `action_catalog.rs` — semantic action definitions and effects.
- `listeners.rs` — listener ownership and consume priorities.
- `bindings.rs` — keyboard, mouse and gamepad button bindings.
- `gamepad_axes.rs` — analog movement/look axes.

Product defaults remain outside the generic bindings API.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-input-profile-gameready`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
