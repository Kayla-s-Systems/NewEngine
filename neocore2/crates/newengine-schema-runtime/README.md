# newengine-schema-runtime

Core-owned baseline provider for `engine.schema`.

This crate is part of the engine foundation, not an externally required plugin.
It registers a normal replaceable gateway route:

```text
engine.schema -> schema.api -> engine.schema.registry
capability: schema.registry
origin: EngineRuntime
```

External first-party/game/mod providers may replace it through the standard
Gateway/Capability registry. The baseline route remains visible in diagnostics
and can become shadowed rather than becoming a hidden singleton or hardcoded
Inspector branch.
