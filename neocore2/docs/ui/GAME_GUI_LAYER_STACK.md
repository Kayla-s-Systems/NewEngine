# Game GUI Layer Stack

`Game GUI` is a provider-neutral runtime composition layer above `engine.ui`.
It borrows the useful idea from UE5's game viewport/game layer manager — viewport content with stable Z-order and optional player-aware layering — without importing Slate/UMG widget ownership into the engine.

The current `game_hud.neui` is intentionally only a **placeholder / smoke-test HUD**. The engine contract is the layer stack, not that HUD document.

## Minimal configuration

The smallest integration keeps the existing single-document project settings:

```json
{
  "profile": "game",
  "game_ui_document_ref": "ui/game/game_hud.neui@surface",
  "game_ui_root_surface_id": "game.hud"
}
```

When no explicit `game_gui` block is present, runtime-host adapts this into:

```text
Game viewport
    -> Game GUI layer stack
        -> hud (z=100, input=game_only)
            -> ui/game/game_hud.neui@surface
```

This is the migration path for existing projects: they get the viewport-layer contract without changing their configuration. The adapter is deliberately a fallback: if a valid `presentation_flow` is active, that flow keeps ownership of its authored screen state and the legacy HUD is **not** mounted a second time. An explicit `game_gui` block remains authoritative when a project intentionally needs viewport layers in addition to presentation flow.

## Full layer configuration

Configure the stack through `engine.runtime.ui.screen_profile.game_gui`:

```json
{
  "profile": "game",
  "game_gui": {
    "enabled": true,
    "layers": [
      {
        "id": "hud",
        "kind": "hud",
        "document_ref": "ui/game/game_hud.neui@surface",
        "surface_id": "game.hud",
        "visible": true,
        "input_mode": "game_only"
      },
      {
        "id": "notifications",
        "kind": "overlay",
        "document_ref": "ui/game/notifications.neui@surface",
        "surface_id": "game.notifications",
        "visible": true,
        "input_mode": "game_and_ui"
      },
      {
        "id": "pause",
        "kind": "menu",
        "document_ref": "ui/game/pause_menu.neui@surface",
        "surface_id": "game.pause",
        "visible": false,
        "input_mode": "ui_only"
      },
      {
        "id": "confirm",
        "kind": "modal",
        "document_ref": "ui/game/confirm.neui@surface",
        "surface_id": "game.confirm",
        "visible": false,
        "input_mode": "ui_only"
      }
    ]
  }
}
```

`z_order` is optional. A zero/omitted value resolves to the engine layer defaults:

- `hud`: 100
- `overlay`: 300
- `menu`: 600
- `modal`: 1000

For this first foundation pass, authored `.neui` surfaces should declare a matching effective z-order. A provider-side z-order override protocol is intentionally deferred.

## Input modes

- `game_only`: UI may render, but the viewport/game owns input.
- `game_and_ui`: UI participates in focus/input without gating gameplay movement or camera navigation.
- `ui_only`: the top visible layer gates gameplay movement and camera navigation.

A visible `modal` layer also publishes modal capture through the shared `UiInputCaptureStateManager`.

## Runtime visibility commands

Gameplay does not call a concrete UI provider. It writes commands into `UiGameLayerCommandQueue`:

```rust
let queue = resources
    .get_mut::<UiGameLayerCommandQueue>()
    .expect("Game GUI command queue missing");
queue.show("pause");
queue.hide("pause");
queue.toggle("map");
```

The runtime host resolves the command against the configured layer id, applies authoritative surface visibility through `engine.ui`, then republishes `UiGameLayerStackState` and input capture for that frame.

## Runtime state

`UiGameLayerStackState` is published into host resources every frame. It contains the resolved layer order, active input mode, top visible layer and top modal surface. Other systems consume this DTO instead of depending on a concrete UI provider.

Authored documents are compiled/mounted through the existing `engine.assets.ui -> engine.ui` path. The Game GUI manager does not render widgets itself and does not depend on the render backend.

## Retained render domains and frame packets

Game UI is no longer flattened into the same retained draw cache as engine/editor/debug UI.
The provider-neutral presentation boundary is split into stable domains:

```text
GameViewport -> Editor -> System -> Debug
```

`UiLayerCompositionPlan` resolves logical surfaces/input/modal ownership. The host turns every
active plan into a `UiLayerDrawPacket` and publishes an ordered `UiLayerDrawPacketSet`. Normal
playable, bootstrap/loading, UI-only tool and degraded-recovery rendering all consume that packet
set directly. There is no singleton `UiDrawList` frame resource.

The render handoff is therefore:

```text
UiLayerCompositionPlan[]
    -> UiLayerDrawPacketSet
    -> RenderFrameEnvelope.ui_layers
    -> ui_composite.game_viewport
    -> ui_composite.editor
    -> ui_composite.system
    -> ui_composite.debug
```

Each generated `UiComposite` pass carries a unique pass id/label and writes the presentation
surface in deterministic domain order. UI is not a scene draw-list kind in render protocol v2;
concrete render backends resolve a composite pass by its domain label and consume exactly the
matching `UiLayerDrawPacket`.

## RenderGraph boundary

The Game GUI stack carries only the logical `viewport_surface_id`. It must not hold a physical texture, descriptor, framebuffer, render-target allocation or backend handle.

The later RenderGraph integration point is deliberately downstream of Phase 3 lifetime analysis:

```text
Live DAG
  -> exact ResourceLifetimeInterval/history
  -> transient allocation / aliasing policy
  -> physical viewport resource binding
  -> Game GUI composite
```

This keeps UI composition stable while the renderer is free to reuse or alias transient physical resources whose live intervals do not overlap.

## Engine-level retained layer domains

The viewport stack is no longer treated as a special HUD cache. `newengine-ui-api`
now exposes a provider-neutral presentation boundary shared by engine subsystems:

```text
UiLayerDomain
  System        -> loading / frontend / fatal-error shell
  GameViewport  -> HUD / overlay / menu / modal
  Editor        -> editor chrome / tooling
  Debug         -> diagnostics / profiler overlays
        |
        v
UiLayerCompositionPlan
  logical target surface
  ordered visible surface ids
  domain invalidation revision
  input owner surface
  modal owner surface
        |
        v
RetainedUiLayerCache(domain)
        |
        v
engine.ui provider
        |
        v
provider-neutral UI composite
```

A domain is a **lifecycle and routing boundary**, not a concrete implementation.
It does not identify egui/Aurelia/Vulkan and does not carry GPU handles.

`GameViewport` currently has its own retained cache independent from `System`.
World frame reconstruction, scene streaming, shadow-cache refresh and other world/render
changes therefore do not own or clear the HUD packet.

### Domain-scoped invalidation

`UiDrawInvalidationState` carries independent epochs for `System`, `GameViewport`,
`Editor` and `Debug` in addition to an aggregate diagnostic revision. Gameplay UI
mutations invalidate only `GameViewport`:

```text
gameplay state patch
    -> engine.ui retained state
    -> invalidate(GameViewport)
    -> next host frame sees a new GameViewport epoch
    -> rebuild only the GameViewport retained layer cache
```

This replaces timer-based HUD reconstruction and prevents unrelated system/editor UI
changes from forcing a gameplay HUD rebuild.

### Composition plan

`UiGameLayerStackState::composition_plan()` converts the authored game stack into the
generic `UiLayerCompositionPlan`. Runtime-host consumes this plan rather than knowing
about `game.hud`, inventory widgets or any gameplay-specific surface ids.

The composition plan also exposes the logical input owner and top modal surface. This
keeps input ownership, retained presentation and render routing based on the same
resolved stack instead of three separate pieces of ad-hoc state.

### Retained cache contract

Each host presentation domain owns `RetainedUiLayerCache` with three invalidation axes:

1. **Topology** — target or ordered visible surface set changed.
2. **Content** — the domain invalidation revision changed.
3. **Animation/forced refresh** — provider output is actively animated or an explicit
   lifecycle/input transition requests refresh.

Cached packets deliberately drop `texture_delta`: atlas upload/free operations are
transport events and must never be replayed from retained UI state.

Render protocol v2 carries multi-domain packets directly. `RenderFrameEnvelope.ui_layers` is
authoritative for engine-owned presentation and RenderGraph expands active domains into ordered
`ui_composite.<domain>` passes. The former `SetUiDrawList` command, `RenderApi::set_ui_draw_list`,
`RenderDrawListKind::Ui` and `newengine-render-ui-bridge` compatibility crate were removed at the
v2 ABI boundary; there is no singleton UI protocol surface left to re-enable.

