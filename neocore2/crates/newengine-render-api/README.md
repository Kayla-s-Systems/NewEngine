# newengine-render-api

Stable render value/protocol contract shared by engine-side bridges and external render plugins.

The crate is backend-neutral. The service id remains `render.api.v1` for discovery compatibility, while the method namespace is versioned:

- `info_json_v1` / `invoke_json_v1` — legacy immediate-mode compatibility path; direct use must produce a runtime warning.
- `info_json_v2` / `invoke_json_v2` — legacy compatibility alias for graph-capable clients; direct use must produce a runtime warning.
- `info_json_v3` / `invoke_json_v3` — canonical foundation protocol for render graph, capability negotiation, diagnostics, upload pacing and V3 immediate compatibility payloads.

## Module layout

- `constants` — service ids and versioned method names.
- `ids` — opaque render resource ids and draw argument types.
- `frame` — frame, viewport and render-target descriptors.
- `resources` — buffers, textures, samplers and shaders.
- `pipeline` — vertex layouts and graphics pipeline descriptors.
- `bindings` — bind group layouts and resource bindings.
- `capabilities` — backend feature/limit declarations.
- `render_graph` — V3 frame graph resources, passes, dependency validation, deterministic compile reports and submit reports.
- `diagnostics` — frame pacing, upload budget, graph stats and resource counters.
- `protocol` — `RenderRequestV1`, `RenderResponseV1`, `RenderImmediateRequest`, `RenderImmediateResponse`, `RenderRequestV3`, `RenderResponseV3`, legacy warnings and JSON helpers.

## V3 contract intent

The V3 layer is modeled after a production renderer split:

`scan/visibility -> draw lists -> render phases -> render graph -> backend executor`

It intentionally does not expose Vulkan/DirectX concepts directly. The engine submits a declarative graph of resources and passes; the backend validates dependencies, compiles a deterministic execution order, pumps uploads according to `RenderWorkBudget`, and reports graph diagnostics back to the system layer.
