# NewEngine Core FSM Contract

## Goal

`EngineFsm` removes split lifecycle ownership from NewEngine. Core owns the application state; all outer layers submit work into core instead of coordinating lifecycle manually.

## State graph

```text
Created -> InitSystem -> InitGame -> Running -> ShutdownGame -> ShutdownSystem -> Stopped
```

`Faulted` is terminal and represents an invalid transition or unrecoverable kernel-level failure.

## Transition policy

| From | Allowed next states |
|---|---|
| `Created` | `InitSystem`, `ShutdownGame`, `Faulted` |
| `InitSystem` | `InitGame`, `ShutdownGame`, `Faulted` |
| `InitGame` | `Running`, `ShutdownGame`, `Faulted` |
| `Running` | `ShutdownGame`, `Faulted` |
| `ShutdownGame` | `ShutdownSystem`, `Faulted` |
| `ShutdownSystem` | `Stopped`, `Faulted` |
| `Stopped` | `Faulted` |
| `Faulted` | terminal |

Invalid transitions force `Faulted` and are logged by `Engine`.

## Shutdown semantics

- `Engine::request_exit()` calls `EngineFsm::request_shutdown()` and signals `ShutdownToken`.
- `ModuleCtx::request_exit()` signals `ShutdownToken`; the engine syncs that token into the FSM before the next callback boundary.
- `Engine::begin_frame()` runs only in `Running` and only while shutdown is not requested.

## Ownership rules

- `EngineFsm` replaces parallel lifecycle flags.
- `ShutdownToken` replaces mutable exit booleans in module contexts.
- Runtime-host/editor/runtime layers may observe `Engine::run_state()`, but may not derive independent lifecycle state from it.
- Job delegation goes through `JobSystemHandle`; engine-thread commits go through `Scheduler`.
