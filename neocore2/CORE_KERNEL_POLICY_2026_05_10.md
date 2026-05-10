# NewEngine Core Kernel Policy — 2026-05-10

## Rule

`newengine-core` is the kernel/orchestrator. It must not own heavyweight platform, image, UI, renderer, importer, or host-diagnostics dependencies by default. Those capabilities belong in plugins or explicit optional features.

## Current enforcement

- `newengine-core` now has `default = []`; no default runtime facade pulls UI/host-probe deps into the kernel.
- `sysinfo` is no longer a default core dependency.
- Windows DXGI/D3D12 host probing is behind `host-probe`.
- Native Windows crash SEH hook is behind `native-crash-handlers`.
- Runtime asset/image decoding remains importer-owned: textures through `imageImporter`, geometry through `geometryImporter`.
- Console typewriter rendering is script-side only and does not affect the Rust kernel dependency graph.

## Cargo usage

Kernel-light build:

```bat
cargo check -p newengine-core --no-default-features
```

Runtime build without heavyweight host probe:

```bat
cargo check -p game-ready-fps
```

Diagnostics-heavy host build, only when explicitly needed:

```bat
cargo check -p newengine-core --features host-probe,native-crash-handlers
```
