# newengine-game-runtime

Standalone game-facing runtime profile for NewEngine.

The crate is the boundary that game binaries should depend on instead of the editor profile.
It disables editor UI markup/panels and allows the render controller to render directly to the platform surface.
