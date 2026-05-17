# Current CoreEngine State: 2026-05-17

CoreEngine now runs as a host/plugin runtime with descriptor-driven gateway routing.

Current healthy launch evidence:

```text
plugins loaded
engine.assets contract ok
engine.render contract ok
engine.physics contract ok
engine state: running
```

Important architectural state:

- Assets are consumed through `engine.assets`.
- Render is consumed through `engine.render`.
- Physics is consumed through `engine.physics`.
- Input is gateway-routable through `engine.input`.
- Provider identity is descriptor-based, not filename-based.
- Missing optional providers degrade by default.
