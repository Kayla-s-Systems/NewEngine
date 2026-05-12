# newengine-render-api

Stable render contract shared by the runtime host, renderer plugins and render-frame-graph builders.

## Service boundary

Renderer plugins expose one service id and two JSON methods:

- service id: `render.api`
- `info_json` — returns `RenderBackendInfo`
- `invoke_json` — accepts `RenderServiceRequest` and returns `RenderServiceResponse`

The runtime submits complete frames through `RenderFrameEnvelope`: frame index, clear color, surface extent, viewport extent, graph, draw-list declarations and optional work budget travel together as one backend-facing packet.

## Modules

- `protocol` — `RenderBackendInfo`, `RenderCommand`, `RenderServiceRequest`, `RenderServiceResponse`, `RenderFrameEnvelope` and JSON helpers.
- `frame_graph` — declarative pass/resource graph descriptions, validation, barriers and submit reports.
- `draw_lists` — typed draw-list routes and replay statistics.
- `provider_bridge` — stable external draw-list/light extraction provider contracts.
- `diagnostics` — backend capabilities, residency, upload pacing and runtime diagnostics.

The renderer contract intentionally has no versioned fallback endpoints. Renderer/plugin/runtime mismatches must be fixed by rebuilding and resyncing the matching module set.
