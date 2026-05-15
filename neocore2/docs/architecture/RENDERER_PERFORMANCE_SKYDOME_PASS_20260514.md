# Renderer Performance + SkyDome Pass — 2026-05-14

## Why the run was slow
The captured run shows the renderer spending most of the prelaunch time in material residency:

- texture requests were started lazily from the render/material path;
- only a small number of material jobs were started per frame;
- CPU texture packets and GPU upload staging were still happening while the launch gate was waiting;
- the first public gameplay frame also had to pay cold-cache costs for terrain/primitive GPU buffers, lit pipelines, and shadows.

The reference renderer in `renderer.zip` is built around a different discipline: draw-list phases are prepared ahead of presentation, resource residency is staged, and expensive visibility/shadow work is not concentrated into the first visible frame.

## What changed

### Material residency
- Split material texture pumping into two budgets:
  - start/import burst budget;
  - decode/GPU upload frame budget.
- Loading-screen prelaunch can now start all declared material imports quickly, while decode/upload stays budgeted.
- Optional environment textures, such as sky textures, are requested but no longer block playable-world release.

### SkyDome
- `skydome/skydome_high.obj` is missing from runtime assets. The engine now falls back to a procedural inward-facing UV skydome mesh instead of dropping the dome.
- Added runtime sky texture `assets/textures/fps/sky_0_runtime.jpg`, generated from the oversized DDS source.
- Updated game-ready scene/default sky material to use `textures/fps/sky_0_runtime.jpg`.

### First-frame performance
- Added GPU resource prewarm while the native loading screen is active:
  - lit pipeline;
  - procedural terrain mesh buffers;
  - primitive/prefab mesh buffers.
- Deferred cold shadow cache population by a couple of frames after first world present.

### Texture budgets
- Material mip-chain cap reduced from 12 to 8.
- Sky/environment textures use one runtime mip and a max runtime dimension cap.
- Oversized runtime texture payloads are downscaled before GPU texture creation.

## Expected effect
- The loading gate should start all scene texture requests earlier.
- The first playable frame should stop absorbing all cold GPU setup costs at once.
- SkyDome should exist even when the OBJ asset is missing.
- SkyDome should use an actual texture instead of falling back to only material color.
