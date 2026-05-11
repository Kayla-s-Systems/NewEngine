# newengine-render-api

Stable render value/protocol contract shared by engine-side bridges and external render plugins.

The crate is intentionally backend-neutral. It defines ids, descriptors, capabilities, frame pacing budgets, diagnostics snapshots and the JSON wire protocol for the `render.api.v1` service.

## Module layout

- `constants` — service ids and wire result aliases.
- `ids` — opaque render resource ids and draw argument types.
- `frame` — frame, viewport and render-target descriptors.
- `resources` — buffers, textures, samplers and shaders.
- `pipeline` — vertex layouts and graphics pipeline descriptors.
- `bindings` — bind group layouts and resource bindings.
- `capabilities` — backend feature/limit declarations.
- `diagnostics` — frame pacing, upload budget and resource counters.
- `protocol` — `RenderRequestV1`, `RenderResponseV1` and JSON helpers.
