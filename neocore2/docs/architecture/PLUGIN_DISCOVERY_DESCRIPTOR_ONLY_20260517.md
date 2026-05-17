# Plugin discovery descriptor-only — 2026-05-17

Runtime plugin discovery is descriptor-first.

```text
candidate dynamic library
  -> ABI probe
  -> PluginSignatureV1 / PluginDescriptor / PluginInfo
  -> declared service/capability metadata
  -> selection
```

The filename is not plugin identity and is not used to classify provider type. A file may be renamed and should still load if its ABI exports and descriptor are valid.

Backend provider selection:

```text
render  = service render.api  + capability render.backend
physics = service physics.api + capability physics.backend
```

If several providers declare the same backend role, selection uses descriptor metadata such as `backend_priority`; path is only a deterministic tie-breaker after equal semantic metadata.

No runtime plugin identity manifest is required for backend discovery.
