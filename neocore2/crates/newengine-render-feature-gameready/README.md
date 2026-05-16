# newengine-render-feature-gameready

Profile-owned render feature pack for the GameReady runtime profile.

This crate owns the GameReady draw-list extraction and light/shadow planning
providers. `newengine-engine-runtime` starts with empty provider registries and
only executes providers explicitly registered by the active profile.
