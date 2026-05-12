# Loading screen real-data/provider pass

## Ownership

The native platform loading screen is a bootstrap/fallback surface. It is rendered by the platform shell because the normal runtime UI provider may not exist yet:

- during platform window creation;
- before engine plugins are loaded;
- before renderer/UI composite is resident;
- during degraded renderer recovery.

So the screen must **not** depend on egui or a renderer-backed UI provider for first paint. It does, however, consume the same provider-style model:

```text
runtime-host / engine resources
  -> newengine-system-contracts::ScreenOverlayStatus
  -> PlatformLoadingOverlayV1.view_json
  -> platform native loading renderer
  -> AssetManager-authored layout/assets
```

## Real data

Stage cards are no longer inferred from `progress_01` in the Win32 renderer. The runtime host publishes structured subsystem data:

- `Platform`
- `Assets`
- `Renderer`
- `Simulation`
- `Diagnostics`

Each subsystem has:

- stable id;
- display label;
- phase: `Waiting`, `Running`, `Ready`, `Degraded`, `Failed`;
- state label;
- optional progress;
- detail string for logs/future inspector UI.

The platform renderer only projects that model. If `view_json` is absent, it uses a compatibility fallback.

## Layout and assets

The visual shell is configured by:

```text
NewEngine/neocore2/assets/loading/loading_screen.layout.json
NewEngine/neocore2/assets/loading/loadBg.png
NewEngine/neocore2/assets/loading/newengine_logo.png
```

All three are resolved through AssetManager. There are no embedded image payloads in the platform plugin.

## UI provider relationship

The loading screen is not egui-provider UI. This is intentional. It is a native bootstrap surface that survives missing/degraded render backends. The unification point is the declarative model and AssetManager-backed layout, not the concrete renderer.

Later, the same `ScreenOverlayStatus` can be rendered by an egui/debug provider inside the editor/runtime after handoff.
