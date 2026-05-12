# Core multitasking pass — 2026-05-12

## Reference patterns extracted from `Core.zip` and `game.zip`

The references are mature because they separate engine concerns by contracts rather than by convenience:

1. **Application FSM**
   - System init, game init, run, game shutdown and system shutdown are distinct phases.
   - Shutdown is not an error path; it is a first-class state transition.

2. **Registry-driven boot/shutdown**
   - Init/shutdown call lists make lifecycle order explicit.
   - Systems do not rely on random update order to become available.

3. **Thread setup as configuration**
   - Threads have roles, priorities and policy instead of ad-hoc spawning.
   - The engine owns concurrency topology.

4. **Time-sliced managers**
   - Streaming/population/gameplay managers are expected to spread work across frames.
   - Frame-critical work and background work must not share an unbounded queue.

5. **Diagnostics channels**
   - Core/game channels make failures attributable.
   - Runtime state should be observable as snapshots, not inferred from log text.

## NewEngine changes in this pass

### Engine FSM

Added `EngineRunState`:

```text
Created -> InitSystem -> InitGame -> Running -> ShutdownGame -> ShutdownSystem -> Stopped
```

The engine now records the state explicitly and exposes `Engine::run_state()`.

### CPU job system

`newengine-core::jobs` is now exported and owned by `Engine`.

The job system has stable lanes:

```text
Simulation
RenderPrep
Streaming
AssetIo
Plugin
Background
```

And priorities:

```text
Critical
Interactive
Normal
Background
```

Modules get the worker pool through:

```rust
ctx.job_system_required()?;
```

No module should spawn its own general-purpose worker pool.

### Engine-thread scheduler

The old two-phase scheduler was replaced with explicit frame phases:

```text
BeginFrame
FixedUpdate
Update
Render
EndFrame
```

Tasks carry a `ScheduleTaskDesc` and `ScheduleBudgetClass`. Non-critical work is time-sliced and deferred instead of blocking the frame.

### Configuration

`EngineConfig` now owns `JobSystemConfig`, so worker count is a kernel-level decision, not an implementation detail hidden inside modules.

### Runtime host cleanup

The platform startup config fallback was renamed from a legacy bridge to startup defaults. It is still a fallback path, but it is no longer represented as a legacy contract.

## Next recommended pass

1. Move asset cooking/import preprocessing to `JobLane::AssetIo`.
2. Move terrain generation and scene contribution lowering to `JobLane::Streaming` or `JobLane::RenderPrep` depending on visibility.
3. Add UI diagnostics panel reading `Engine::job_system_snapshot()` and `Scheduler::snapshot()`.
4. Replace remaining broad plugin/runtime fallback paths with strict service contracts.
