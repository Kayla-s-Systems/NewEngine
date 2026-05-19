# newengine-render-feature-gameready

Profile-owned GameReady render feature pack.

This crate implements `newengine-render-feature-api` providers for terrain,
primitive mesh/UI draw-list extraction and GameReady light/shadow policy. It has
no dependency on `newengine-engine-runtime`; the product profile composes the
returned providers into the active runtime controller.

The renderer backend remains replaceable behind `render.api`. This crate owns
profile policy, not backend submission or runtime controller state.
