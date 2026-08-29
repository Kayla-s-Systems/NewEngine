# newengine-ui-api

Stable engine-facing UI service contract for `engine.ui`.

The UI gateway is owned by the engine host. Concrete UI providers implement `ui.api` or a descriptor-declared provider service and advertise `ui.backend` metadata. Render, camera and gameplay systems publish UI-neutral telemetry/state to `engine.ui`; they do not talk to concrete UI providers.

## Game GUI foundation

Game UI is modeled as a provider-neutral viewport layer stack rather than a renderer-owned HUD. The screen profile owns the logical viewport surface, `UiGameGuiConfig` declares authored `.neui` layers, and `UiGameLayerStackState` publishes the resolved z-order, visibility and input ownership for runtime consumers.

A minimal HUD + pause menu can be declared without backend-specific handles:

```rust
let game_gui = UiGameGuiConfig::enabled()
    .with_layer(UiGameLayerDescriptor::hud(
        "hud",
        "ui/game/hud.neui@surface",
        "game.hud",
    ))
    .with_layer(
        UiGameLayerDescriptor::menu(
            "pause",
            "ui/game/pause.neui@surface",
            "game.pause",
        )
        .initially_hidden(),
    );
```

Layer kinds use stable default z-order bands (`Hud`, `Overlay`, `Menu`, `Modal`) and input modes mirror the useful game/UI ownership semantics: `GameOnly`, `GameAndUi`, and `UiOnly`. Runtime code can show, hide or toggle layers through `UiGameLayerCommandQueue` without reaching into a concrete provider.

The viewport identity carried by `UiGameLayerStackState` is deliberately logical. No transient texture/allocation handle is exposed through this API. RenderGraph physical resource binding, transient allocation and aliasing should be connected only after compiler lifetime analysis has authoritative live resource history.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-ui-api`

**Role:** User interface runtime assets or UI provider implementation data.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
