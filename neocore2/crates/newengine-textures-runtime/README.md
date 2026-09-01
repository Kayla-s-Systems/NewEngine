# newengine-textures-runtime

Provider-neutral `textures.api` semantic service implementation for `.ytd` texture dictionaries.

This crate builds texture semantics but does not own YTD file-type discovery. StarVault 3.8.0 discovers the dedicated `ytd.dll/.so/.dylib` descriptor module from its relative `formats/` directory. That module owns the `.ytd` `AssetFileTypeDescriptor` and points semantic consumers at `engine.assets.textures`.

Boundary rule:

```text
pluginsRuntime/formats/ytd.*
    -> owns YTD asset-type descriptor / format identity

newengine-textures-runtime
    -> owns texture semantic service implementation

StarVault / engine.assets
    -> owns VFS bytes, asset-type registry and format-module discovery

AssetManager codec workers
    -> own byte/ListFile decode mechanics

renderer/UI/materials
    -> consume texture packets or validation DTOs, never raw .ytd bytes
```
