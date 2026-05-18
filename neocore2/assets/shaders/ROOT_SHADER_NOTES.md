# NewEngine root runtime shaders

This directory contains the canonical runtime shader entrypoints loaded by the
current Vulkan render provider through AssetManager.

Updated root shader goals:

- keep the render API and descriptor layouts unchanged;
- keep GLSL sources self-contained because the runtime baker writes transient
  sources into the shader cache before calling `glslc`;
- improve the current forward root without copying external ReShade/ENB/RAGE
  source code verbatim;
- use the uploaded shader pack as a visual/architectural reference for bloom,
  filmic tonemapping, sun optics, natural color grade, terrain anti-tiling and
  stable soft shadow filtering.

Canonical files:

- `game_lit_shadowed_v1.vert/.frag` — single-object lit PBR root;
- `game_lit_instanced_v1.vert/.frag` — instanced lit PBR root;
- `game_terrain_surface_v1.frag` — GameReady terrain root;
- `game_sun_shadow_depth_v1.*` and `game_sun_shadow_depth_instanced_v1.vert` — sun shadow roots;
- `postfx/fullscreen_triangle.vert` and `postfx/tonemap_display.frag` — HDR-to-display root.

Important constraint: do not add `#include` dependencies to these runtime GLSL
files until the baker has a real shader virtual-include resolver. The current
safe contract is one logical shader asset equals one self-contained source file.

## 2026-05-18 effect shader parity pass

`assets/shaders/effects` is now organized as a small native effect shader pack rather than minimal placeholder shaders.
The GLSL sources borrow structure from professional multi-pass post-processing stacks, but are rewritten for NewEngine's Vulkan/ShaderRegistry contract:

- `common.glsl` contains shared luminance, soft-threshold, tent sampling, temporal clamp and PCSS helpers.
- Bloom is split into extract/downsample/upsample/composite passes with radius, knee, intensity and blend defines.
- TAA uses bicubic history sampling, neighborhood min/max clamp and depth rejection hooks.
- MSAA resolve supports explicit sample-count variants and optional gamma-correct accumulation.
- PCSS shadows use blocker search + Vogel disk filtering and shadow bias defines.
- Tessellation uses distance-sensitive factors and runtime displacement scale defines.

The runtime still compiles through provider-owned `ShaderRegistry`; these shader files are content assets loaded through `engine.assets`.
