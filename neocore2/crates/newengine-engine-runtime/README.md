# newengine-engine-runtime

Thin runtime composition and compatibility facade for North Star Engine.

## Ownership invariant

This crate may compose capability runtimes and preserve stable compatibility re-exports, but it
does not own gameplay, render, scene, camera, audio, physics, UI, or world-authority semantics.
Those implementations live in dedicated crates. New domain behavior must be added to the owning
capability/runtime crate rather than growing this composition root.
