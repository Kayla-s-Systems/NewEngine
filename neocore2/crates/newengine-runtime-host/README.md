# newengine-runtime-host

Platform-facing runtime shell and typed adapters over engine gateways.

## Architecture notes

This crate is part of the North Star Engine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-runtime-host`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
## Void Engine default composition

`newengine-runtime-host` has an empty default feature set. A plain dependency links only the core/host-kernel floor; windowed/product orchestration is an explicit `full-runtime` opt-in. Concrete boot presentation (`prestart-window-egui`), asset bootstrap policy, world-authority routing, renderer/physics providers and gameplay composition are not kernel defaults.

Lower domain runtimes must not depend upward on `newengine-runtime-host`; shared domain policies belong in dedicated runtime/adapter crates.

The optional `command-console` feature installs `newengine-console-runtime`
as the provider route `engine.command -> newengine.console.command`.
`newengine-core` and `newengine-host-kernel` never construct that provider.

## Host PreInit pipeline

Windowed/product compositions execute a host-owned PreInit before `Engine` construction:

```text
Executable
    -> Host Bootstrap
    -> Environment Snapshot
    -> Platform Services
    -> Hardware Discovery
    -> Capability Resolution
    -> Engine construction
    -> Runtime Composition
    -> Game Module
```

`newengine-host-capabilities-api` owns only immutable DTOs. `newengine-host-capabilities-runtime` owns OS/hardware probing. `newengine-runtime-host` derives and installs generic gateway-selection policy before provider resolution, then inserts `Arc<HostPreInitSnapshot>` into Engine resources and passes `&HostPreInitSnapshot` explicitly to the runtime-composition hook. Runtime/domain code must consume that snapshot instead of probing hardware again.

Provider selection uses normalized metadata tags such as `backend.vulkan` / `backend.d3d12`; the Host may prefer or reject tags based on OS/hardware compatibility without naming a concrete provider plugin.
