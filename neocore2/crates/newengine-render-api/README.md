# newengine-render-api

Stable render value/protocol contract shared by engine-side bridges and external render plugins.

The crate is intentionally backend-neutral. It defines ids, descriptors, capabilities, frame pacing budgets, diagnostics snapshots and JSON wire protocols for the `render.api.v1` service.

## Compatibility rule

`RenderRequestV1` / `RenderResponseV1` are append-only only when absolutely safe. New behavior that can change backend requirements goes through `RenderRequestV2` / `RenderResponseV2` and the explicit service methods:

- `info_json_v2`
- `invoke_json_v2`

Old plugins can continue to serve v1. Engine-side bridges may locally validate/compile render graphs when a v1-only backend is present, but execution requires v2 capability negotiation.

## Render graph layer

`render_graph` describes a frame as a directed acyclic graph of passes and logical resources:

- resources are `Persistent`, `TransientFrame`, bounded `Frames(n)`, or `External`;
- passes declare reads/writes and queue class (`Graphics`, `Compute`, `Transfer`);
- graph compilation returns deterministic execution order, lifetime stats, estimated aliasing savings and barrier counts;
- submit reports include upload pumping and pass execution telemetry.

This layer is the future path for transient resource aliasing, upload budgeting, automatic barriers and timeline semaphore placement.

## Error contract

V2 supports `RenderProblemDetails`, an RFC 9457-style machine-readable error envelope with:

- stable `code`;
- human `title` / `detail`;
- `phase`;
- `backend`;
- `retryable`.

Runtime code should branch on `code`/`phase`, not parse human text.

## Module layout

- `constants` — service ids and v1/v2 wire method names.
- `ids` — opaque render resource ids and draw argument types.
- `frame` — frame, viewport and render-target descriptors.
- `resources` — buffers, textures, samplers and shaders.
- `pipeline` — vertex layouts and graphics pipeline descriptors.
- `bindings` — bind group layouts and resource bindings.
- `capabilities` — backend feature/limit declarations and v2 capability flags.
- `diagnostics` — frame pacing, upload budget, graph stats and resource counters.
- `render_graph` — graph resources, pass declarations, validation and deterministic compilation.
- `protocol` — `RenderRequestV1/V2`, `RenderResponseV1/V2`, capability negotiation and problem details.
