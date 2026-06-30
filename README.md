<p align="center">
  <img src=".github/NorthStarBanner.png" alt="NorthStar Engine banner" width="100%">
</p>

# NorthStar Engine

[![CI](https://img.shields.io/badge/CI-GitHub%20Actions-2088ff?logo=githubactions&logoColor=white)](.github/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-stable%20%7C%20nightly-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Platforms](https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS-lightgrey)](#continuous-integration)
[![Architecture](https://img.shields.io/badge/architecture-host%2Fplugin-blueviolet)](NewEngine/neocore2/docs/CoreEngine_Documentation/15_GATEWAY_SERVICE_LAYER.md)
[![Status](https://img.shields.io/badge/status-pre--alpha-yellow)](#project-status)

**NorthStar Engine** is the Take Some() modular runtime and rendering technology stack for game-engine infrastructure.
This repository contains **NewEngine / CoreEngine**: a Rust game-engine runtime built around a strict host/plugin architecture.
The engine owns lifecycle, scheduling, resources, startup validation, and service routing.
Feature implementations live in replaceable plugins.

```text
consumer -> engine.* gateway -> active provider service -> typed adapter -> runtime systems
```

## Project identity

| Field | Value |
| --- | --- |
| Product | NorthStar Engine |
| Organization | Take Some() |
| Repository | NewEngine |
| Core architecture | Host/plugin runtime with stable engine gateways and replaceable providers |
| Primary implementation | Rust workspace under `NewEngine/neocore2` |

## Why this engine exists

NorthStar Engine is designed to avoid becoming a wrapper around one renderer, one physics SDK, or one asset manager.
The host exposes stable engine-facing gateways such as `engine.render`, `engine.physics`, `engine.assets`, and `engine.input`.
Provider plugins describe themselves through metadata, and the host selects the active implementation from descriptors.

## Architecture at a glance

```text
newengine-service-api      service vocabulary, gateway ids, runtime requirement specs
newengine-plugin-api       stable plugin ABI and service descriptors
newengine-plugin-host      descriptor loading, gateway registry, provider selection
newengine-core             lifecycle, startup graph, Resources, declarative validation
newengine-runtime-host     platform shell and typed domain adapters
newengine-engine-runtime   gameplay/runtime orchestration without backend ownership
Plugins/*                  renderer, physics, assets, input, platform, logging providers
```

Key invariants:

- provider identity comes from ABI descriptors, not file names;
- consumers call `engine.*` gateways, not provider-owned service ids;
- backend priority is declared in metadata;
- missing optional providers degrade by default;
- strict startup behavior is explicit runtime policy;
- engine-runtime does not import concrete provider implementation crates.

## Gateway Service Layer

The Gateway Service Layer is the runtime-hosted facade over plugin services.
A provider may register `render.api`, `vendor.render.api`, or another private service id.
Consumers still call `engine.render`; the host resolves that gateway to the active provider.

```json
{
  "service_kind": "render",
  "engine_gateway": "engine.render",
  "contract": "render.api",
  "backend_priority": 100
}
```

Read the full design note: [`15_GATEWAY_SERVICE_LAYER.md`](NewEngine/neocore2/docs/CoreEngine_Documentation/15_GATEWAY_SERVICE_LAYER.md).

## Repository layout

```text
NewEngine/neocore2/        main Rust workspace
Plugins/                   runtime provider workspaces
Importers/                 importer plugins and build support
tools/                     asset/package authoring tools
```

## Build

Core workspace:

```bash
cd NewEngine/neocore2
cargo build
```

Run the game-ready sample:

```bash
cd NewEngine/neocore2
cargo run --bin game-ready-fps --profile dev
```

Build plugins from the repository root:

```bat
Plugins\build_all_plugins.cmd
```

## Continuous integration

The repository includes a GitHub Actions workflow at `.github/workflows/ci.yml`.
It checks formatting, Clippy, builds, library tests, documentation tests, Miri on nightly, and coverage on Linux.

## Documentation

Start here:

- [`Architecture overview`](NewEngine/neocore2/docs/CoreEngine_Documentation/01_ARCHITECTURE_OVERVIEW.md)
- [`Provider/adapter guide`](NewEngine/neocore2/docs/CoreEngine_Documentation/03_PROVIDER_ADAPTER_SYSTEM_GUIDE.md)
- [`Gateway Service Layer`](NewEngine/neocore2/docs/CoreEngine_Documentation/15_GATEWAY_SERVICE_LAYER.md)
- [`.ytd texture dictionary`](NewEngine/neocore2/docs/codecs/YTD_TEXTURE_DICTIONARY.md)
- [`.nepak asset package`](NewEngine/neocore2/docs/CoreEngine_Documentation/13_NEPAK_CODEC_SPEC.md)

## Project status

NorthStar Engine is pre-alpha engine infrastructure.
The architecture is intentionally strict and release-oriented, but APIs may still move while the runtime, plugin SDK, conformance tests, and content tools mature.

## Contributing

Contributions should preserve the host/plugin boundary.
Avoid provider-specific branches in engine-runtime and prefer descriptor-driven routing over manual service-name checks.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine`

**Role:** Core engine source area. This is where the engine host, runtime crates, config, editor, and core scan scripts live.

**Local contents:** 3 direct subdirectories, 5 direct files.

**Direct file examples:** `.gitignore`, `banner.png`, `CODE_OF_CONDUCT.md`, `LICENSE`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
