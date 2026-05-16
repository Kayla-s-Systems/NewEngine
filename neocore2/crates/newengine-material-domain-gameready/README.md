# newengine-material-domain-gameready

GameReady/FPS material-domain package.

It owns the current runtime-lit shader paths, fallback textures, samplers and
pipeline presets. Reusable `newengine-engine-runtime` registers this provider
through `newengine-material-domain-api` instead of hardcoding GameReady material
assets inside the render controller.
