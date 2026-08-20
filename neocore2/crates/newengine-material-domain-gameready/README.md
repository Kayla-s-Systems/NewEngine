# newengine-material-domain-gameready

Compatibility facade for existing GameReady consumers. The lit material implementation, shader-manifest loading, staged pipeline warmup and cache ownership live in `newengine-material-domain-standard`.

The historical `GameReadyLitMaterialDomainProvider` and `GAME_READY_LIT_PIPELINE_KEY` remain available so existing profiles and tools do not need an immediate source/API migration.
