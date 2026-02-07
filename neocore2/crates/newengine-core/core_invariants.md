# NewEngine Core Invariants

Core is a **deterministic orchestrator** and **ABI firewall**.

Core does not implement subsystems.
Core guarantees:

- Lifecycle ordering
- ABI compatibility validation
- Isolation (panic containment + disablement)
- Deterministic time model (ticks/frame-index/delta)
- Shutdown ownership

If an invariant cannot be enforced in code, the corresponding API **must not exist**.
If an invariant is violated at runtime, the core **must panic**.

## Invariants

### I1. No direct calls into implementations

The core never calls subsystem implementations.
Subsystems are loaded and executed only through the plugin lifecycle contract.

### I2. Core does not own plugin state

Core does not store subsystem state.
Only minimal lifecycle state is allowed (FSM stage + id + error reason).

### I3. Core does not allocate on behalf of plugins

Memory ownership across ABI is explicit. Core never "creates" plugin-owned objects.

### I4. Core owns time

Plugins are driven by `frame_index`, `tick` and `dt` (delta).
Plugins must not rely on wall-clock time.

### I5. Core owns shutdown

Shutdown is initiated and coordinated by the core.
Plugins are not allowed to keep the host alive indefinitely.

### I6. No infinite waits

Core must not block forever waiting for plugin code.
Plugin entrypoints are panic-contained; failures disable the plugin.

### I7. ABI is strict

ABI version mismatch == load refusal.
No "best effort" fallback.

### I8. Core is not a message bus between plugins

Core is not a service locator and not an RPC router.
Plugins must not depend on core-mediated inter-plugin calls.

> Note: legacy bridging APIs may exist temporarily, but must be marked as legacy
> and scheduled for removal once plugin-to-plugin coupling is moved to explicit
> contracts.