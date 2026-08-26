# Project-Owned Frontend Contract

North Star owns the **UI runtime**, not a game's menu.

## Ownership boundary

Engine/runtime owns:

- NEUI decoding, layout, input/focus routing and rendering.
- presentation-state execution and runtime-ready signalling.
- generic system surfaces (loading/error/editor tooling).
- action transport (`game.start`, `ui.back`, pause toggles, lifecycle commands).

A project owns:

- main menu, HUD, pause menu, credits and product-specific settings composition.
- presentation state ids and transitions.
- which states block world bootstrap or gameplay input.
- the initial presentation state for each launch preset.

The declaration lives in `game.toml` under `[ui.presentation_flow]`, `[[ui.presentation_flow.states]]` and `[[ui.presentation_flow.transitions]]`. UI documents referenced by a project flow must not use `ui/engine/...`; they are resolved from project content mounts.

## Runtime model

`newengine-project-api` validates the authored graph. `newengine-project-runtime` projects the complete graph into the generic `engine.runtime` screen-profile configuration before plugin startup. `newengine-windowed-host-runtime` executes the graph without knowing names such as `main_menu`, `pause`, or `credits`.

This keeps the engine reusable: a project may boot directly to gameplay, provide one menu, or define an arbitrary frontend state graph without changing engine code.
