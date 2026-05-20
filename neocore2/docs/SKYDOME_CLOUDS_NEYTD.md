# SkyDome, Sun Cycle and Cloud Texture Dictionaries

## Runtime contract

SkyDome remains scene/profile-owned data and render-provider execution stays behind `engine.render`. Runtime only describes intent through material texture references, lighting state and postfx params; the renderer owns native shader compilation, GPU resources and post-processing.

## Infinite SkyDome

The GameReady SkyDome now carries a `SkyDomeRuntime` marker. The render extraction path recenters marked sky primitives to the active camera before submitting instance data. This makes the sky effectively infinite without adding collision hacks or allowing the player/camera to visually leave the dome.

Sky primitives are also excluded from shadow submission regardless of material settings.

## Day/night cycle

`lighting.day_night` controls the runtime solar cycle:

```json
"day_night": {
  "enabled": true,
  "time_of_day_hours": 9.35,
  "day_length_seconds": 720.0,
  "latitude_degrees": 45.0,
  "axial_tilt_degrees": 23.44
}
```

The cycle updates the active `DirectionalLight` direction, color and intensity every frame and also modulates ambient light for night, dawn/dusk and daylight.

## Sun optics

The postfx sun path now derives visibility, disk radius, flare strength and ray strength from projected screen position plus solar elevation. Looking near the sun produces stronger disk/corona/ray/ghost effects, while low-angle sun gets warmer, wider and more optical scattering.

## Cloud dictionary

All source DDS cloud types from `clouds.zip/clouds/*` are packed into:

```text
assets/textures/fps/clouds_runtime.neytd
assets/textures/fps/clouds_runtime.manifest.json
```

Runtime scene/material references must use dictionary selectors, for example:

```text
textures/fps/clouds_runtime.neytd@cloud_clear__new_skyhat_clear01_bot_ap
textures/fps/clouds_runtime.neytd@cloud_clear__new_skyhat_clear01_bot_nrm
```

The manifest groups entries by profile/type:

```text
alt_heavy, alt_light, alt_med, cirrus, clear, cloudy, contrails,
horizon, nimbus, puffs, rain, showers, storm, test, wispy
```

The dictionary keeps compressed GPU-ready mip chains: DXT1 sources become `BC1_RGBA_UNORM`; BC5U/ATI2 sources become `BC5_RG_UNORM`. Cloud textures are stored as linear data because they are density/coverage/normal maps rather than final sRGB albedo.

## HLSL source path

Provider-ready HLSL helpers were added under:

```text
assets/shaders/sky/skydome_atmosphere.hlsl
assets/shaders/sky/skydome_cloud_layer.hlsl
```

The current GameReady pass still works through the existing lit primitive/postfx pipeline; these HLSL files are the clean next split point for a dedicated provider-owned sky pass.
