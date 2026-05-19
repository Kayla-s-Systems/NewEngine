# UI contrast and model construction domains — 2026-05-19

## Pause menu rendering

The pause menu is a full-screen modal surface. The world is dimmed by a provider-side full-screen backdrop and by render-graph `ui_backdrop_blur` intent. Selection hit-testing must use the same layout constants as drawing, so mouse hover, keyboard selection and the visual highlight target the same row.

Current fixes:

- full-screen dim layer raised to high opacity;
- panel and info rail use opaque dark surfaces for readable contrast;
- selected row geometry aligns with item layout;
- text now uses a single glyph advance plus shadowed rendering instead of excessive spacing;
- hover detection shares panel/list constants with the drawn layout.

## Model domains

Model construction is split into explicit subdomains, matching the same third-level gateway pattern as input/render/physics/camera:

```text
engine.model             <- composed model assembly
engine.model.skeletons   <- skeleton metadata / rig anchors
engine.model.materials   <- model material set references
engine.model.collisions  <- collision proxies for model bodies
```

The runtime adapter path remains AssetManager-backed:

```text
model manifest/request
  -> model logical asset
  -> skeleton logical asset
  -> material set
  -> .neytd texture dictionary references
  -> collision descriptors
  -> ModelAssetBundle
```

The adapter returns data-only mesh/material/skeleton/collision bindings. It does not own ECS, renderer backend handles or physics backend handles.

## Related existing domains

```text
engine.input.bindings      keybind configuration
engine.input.actions       interpreted gameplay actions
engine.input.contexts      context-aware input handling
engine.render.effects      post-process effects
engine.render.materials    render material pipeline/material domain
engine.physics.contacts    collision handling
engine.physics.constraints joint/constraint system
engine.camera.modes        camera behavior modes
engine.camera.animations   camera transitions
```
