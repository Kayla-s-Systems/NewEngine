# NewEngine Core Invariants

Core is a **deterministic orchestrator**, **thread provider** and **ABI firewall**.

Core does not implement subsystems. Core guarantees:

- Lifecycle ordering through `EngineFsm`
- ABI version validation
- Module/plugin isolation
- Deterministic time model (`frame_index`, fixed tick, delta)
- Shutdown ownership
- CPU work delegation through one `JobSystem`
- Engine-thread work delegation through one `Scheduler`

If an invariant cannot be enforced in code, the corresponding API must not exist.

## Invariants

### I1. One lifecycle source

`EngineFsm` is the only lifecycle source of truth. Runtime-host, editor, gameplay runtime and modules must not mirror it with local lifecycle flags.

### I2. Core owns shutdown

Shutdown is initiated/coordinated by core. Modules can request shutdown through `ModuleCtx::request_exit()`, but they never mutate core lifecycle state directly.

### I3. Core owns thread delegation

CPU-heavy work goes through `JobSystemHandle`. Engine-thread commits go through `Scheduler`. Subsystems must not create hidden worker pools for asset IO, streaming, render-prep, simulation or plugin tasks.

### I4. Core does not own subsystem state

Core stores only minimal lifecycle/module state: FSM state, module slot state, readiness state and error reason. Subsystem domain state belongs to modules/plugins.

### I5. No direct calls into implementations

Subsystems are loaded and executed through explicit module/plugin/service contracts. Core does not call backend implementation internals directly.

### I6. Core owns time

Plugins and modules are driven by `frame_index`, fixed tick and `dt`. They must not use wall-clock time as simulation authority.

### I7. No infinite waits

Core must not block forever waiting for module/plugin code. Callback failures are contained and routed through strict/resilient fault policy.

### I8. ABI is strict

ABI version mismatch is load refusal. No fallback protocol path is allowed in core.

### I9. Core is not an inter-plugin bus

Core exposes service boundaries and events, but must not become a hidden plugin-to-plugin coupling layer.
