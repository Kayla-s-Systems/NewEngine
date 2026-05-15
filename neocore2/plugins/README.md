# NewEngine plugins directory

This directory is the runtime plugin deployment root.

## Dynamic libraries

Runtime DLLs are discovered from this directory and from `importers/` according
to `plugins.manifest.json`.

Naming convention:

```text
<plugin-id-or-short-name>-<semver>-<profile>.dll
```

Examples:

```text
logging-0.2.10-release.dll
vulkan_renderer-0.3.4-release.dll
importers/textureImporter-0.2.4-dev.dll
```

## Declarative content

`plugins.manifest.json.content` is the plugin-owned content catalog. The engine
reads this catalog before falling back to built-in dev defaults.

Current content owners:

- `newengine.game_ready.map` owns `newengine.scene.game_ready.highlands.v1`.
- `newengine.assets` owns prefab references such as `prefabs/tree_animate/scene.gltf`.
- importer DLLs own asset decoding; the engine should not duplicate importer logic.
