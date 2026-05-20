# Aurelia Shader Bootstrap Loading Contract — 2026-05-20

## Problem

The white startup window was not an Aurelia styling problem. It was a startup-contract problem:

- the OS window became visible before the first renderer-owned UI frame;
- shader/pipeline creation could block the first normal render path;
- loading progress jumped to the final handoff range before shader and scene residency were actually visible;
- missing shader cache was treated like a visual dead zone instead of a normal loading phase.

## Contract

A first run with an empty shader/pipeline cache is a valid runtime state. It must be visible in `engine.loading` and projected into Aurelia through `engine.ui`.

The user should see states such as:

- Checking shader cache…
- Compiling missing shaders and pipelines…
- Building render pipelines…
- Preparing playable world…
- textures ready N/M

## Ownership

- `platform-winit` owns window/input only.
- `engine.loading` owns loading state.
- `engine.ui` / Aurelia owns visual loading surfaces.
- Vulkan owns renderer execution and pipeline cache, but reports diagnostics through renderer-neutral `RenderDiagnosticsSnapshot`.
- The runtime/render controller maps renderer diagnostics into loading state without letting Aurelia touch Vulkan internals.

## Changes in this pass

- Bootstrap progress no longer jumps to 95–97% before the visible loading surface.
- Scene-launch loading starts in the mid range and advances with texture residency.
- Shader/pipeline diagnostics are reflected in loading details.
- UI-only loading frames use zero upload budget.
- Scene/material upload warmup is throttled during the first visible Aurelia frames.
- Vulkan upload queue now treats zero budget as zero work.
- Legacy renderer-owned text/core pipeline creation is deferred so it cannot block the first Aurelia present.

## Invariant

Aurelia remains a provider behind `engine.ui`. Vulkan receives only draw packets and renderer-neutral state; it never receives XML, widget ownership, or UI domain authority.
