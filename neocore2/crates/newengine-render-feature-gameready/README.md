# newengine-render-feature-gameready

Compatibility facade over `newengine-render-feature-standard`. The standard crate is the sole owner of draw-list extraction and light/shadow extraction.

`GameReadyRenderFeaturePack` remains available and wraps the standard providers with the historical `gameready.*` IDs so existing profile diagnostics and integrations keep their identifiers while implementation ownership is generic.
