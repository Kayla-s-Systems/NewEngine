# Logging and Diagnostics Policy

Runtime logs must describe state changes, not repaint the whole engine every frame.

## Levels

| Level | Use |
| --- | --- |
| `ERROR` | Startup/runtime contract failure that prevents the requested mode. |
| `WARN` | Degraded backend, slow sampled frame, rejected command, soft timeout. |
| `INFO` | One-time lifecycle milestones, selected providers, compact route tables, explicit user-requested snapshots. |
| `DEBUG` | Per-provider paths, per-frame timing breakdowns, input transitions, UI frame telemetry, full contract methods. |

## Default runtime shape

The normal startup log should be readable in one pass:

```text
plugins: done (...)
Plugins :: Registered             # compact: id/ver/state
Plugins :: Gateway Routes         # compact: gateway/state/source/provider/prio
runtime api                       # compact: api/status/provider/source/strict
input systems: initialized ...    # compact catalog line
```

Verbose details are still available through `DEBUG`, but are no longer emitted at `INFO`:

- full plugin owner/capability/score columns;
- runtime contract required method lists;
- draw-list/light provider labels;
- UI gateway frame payload stats;
- per-input-frame summaries;
- individual model/prefab mesh part registrations.

## Frame-loop rule

A frame-loop log must be one of:

1. a transition (`active false->true`, capture released, backend changed);
2. a sampled diagnostic (`frame % N == 0`);
3. a slow-frame warning with rate limiting;
4. an explicit snapshot requested by tooling/user code.

It must not emit identical state every frame. Modal capture surfaces, such as `engine.pause_menu`, keep capture state persistent so they do not alternate `captured=false` and `captured=true` on every frame.

## Input snapshots

Use explicit snapshots when diagnosing input domains:

```rust
render_controller.log_input_systems_snapshot("before cutscene");
let snapshot = render_controller.input_systems_snapshot();
```

Default logs only show the compact system catalog and `DEBUG` transitions. This keeps the useful ability to see active/disabled/captured systems without turning gameplay frames into log spam.
