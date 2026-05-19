# Domain hierarchy and gateway levels — 2026-05-19

## Rule

Engine gateway ids and `service_kind` must describe the same domain depth.

Valid examples:

```text
engine.input            + input
engine.input.bindings   + input.bindings
engine.input.actions    + input.actions
engine.input.contexts   + input.contexts

engine.render           + render
engine.render.effects   + render.effects
engine.render.materials + render.materials

engine.model             + model
engine.model.skeletons   + model.skeletons
engine.model.materials   + model.materials
engine.model.collisions  + model.collisions

engine.physics              + physics
engine.physics.contacts     + physics.contacts
engine.physics.constraints  + physics.constraints

engine.camera             + camera
engine.camera.modes       + camera.modes
engine.camera.animations  + camera.animations
```

Invalid examples:

```text
engine.input.bindings + input
engine.render.effects + render
engine.physics        + physics.contacts
engine.camera.modes   + camera
```

The host now rejects mixed parent/child route metadata before it enters the active gateway registry.

## Why

A second-level gateway is the parent facade. A third-level gateway is a bounded extension surface owned by that parent domain. This keeps domain growth additive instead of turning the gateway registry into an unstructured flat bag of ids.

```text
engine.input          raw input frame domain
  engine.input.bindings   keybind/profile configuration
  engine.input.actions    interpreted gameplay/menu/camera actions
  engine.input.contexts   future context stack and focus arbitration

engine.render         frame submission backend
  engine.render.effects   post-process/effect stack providers
  engine.render.materials material system providers

engine.model          composed runtime model construction
  engine.model.skeletons  skeleton metadata and humanoid anchors
  engine.model.materials  model material set and .neytd references
  engine.model.collisions collision proxies derived from model/rig data

engine.physics        simulation backend
  engine.physics.contacts collision/contact event handling
  engine.physics.constraints joint/constraint system

engine.camera         camera runtime/provider
  engine.camera.modes      behavior mode providers
  engine.camera.animations transition/animation providers
```

## Implementation points

- `EngineServiceKind` is now canonical for both parent and child domains.
- `EngineServiceKind::engine_gateway_id()` returns the only accepted gateway id for a kind.
- `EngineServiceKind::parent()` expresses domain ownership for third-level domains.
- Plugin capability metadata is rejected when `service_kind` and `engine_gateway` do not match.
- Engine-owned routes pass through the same validation path as plugin routes.
- Override mode remains policy-driven: root render/physics/assets/scene/model construction stay profile-controlled, while input/camera/render-effects/render-material extension surfaces are open unless a future profile policy clamps them.


## Input registration rule

`engine.input.bindings` is the only domain allowed to register physical inputs and semantic action declarations. A system that wants a key, mouse button, gamepad button or gamepad axis must register the action and binding there instead of polling raw input or adding local `if key == ...` branches.

```text
feature/provider/system
  -> register_action_json_v1(action id + data-driven effects)
  -> register_binding_json_v1(key/mouse/gamepad binding)
  -> register_listener_json_v1(listener ownership/filter metadata)
  -> runtime resolves InputActionFrame from profile + raw engine.input snapshot
```

The profile contains keyboard, mouse and gamepad bindings plus axis mappings. The resolver applies action effect descriptors, not hardcoded action-id switch logic.

## Player runtime asset rule

Player model geometry is loaded through `engine.assets`. Player material textures must be referenced as entries in one `.neytd` dictionary, for example:

```text
player/abigail/textures/abigail.neytd@hair_diff_000_a_uni
player/abigail/textures/abigail.neytd@head_normal_000
```

Runtime must not probe many single-texture `.neytd` files such as `hair_diff_000_a_uni.neytd`; `.neytd` is a texture dictionary container.

## Player human rig rule

The player model profile now carries `target_height` and `eye_height_ratio`. Runtime derives the first-person camera offset from the model's feet-to-eye height and the player capsule origin, so the camera attaches at human eye level rather than at capsule top/head top.

## Model construction rule

The model pipeline is now expressed as a model-domain hierarchy rather than as player-specific loader code. The intended composition is:

```text
engine.model request/manifest
  -> engine.model.skeletons metadata / anchors
  -> engine.model.materials material set / .neytd dictionary entries
  -> engine.model.collisions collision proxies
  -> runtime ModelAssetBundle
```

`newengine-model-domain-api` owns the gateway vocabulary and declarative manifest DTOs. `newengine-model-adapter` resolves the current AssetManager-backed implementation and returns backend-neutral data.
