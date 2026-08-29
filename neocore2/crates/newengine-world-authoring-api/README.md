# newengine-world-authoring-api

Provider-neutral authored-world placement contract.

Owns only authored placement identity and transient authoring markers:

- `AuthoredMapPlacement`
- `AuthoredMapPlacementSource`
- `AuthoredMapPlacementDirty`
- `AuthoredMapPlacementCloneSource`
- `AuthoredMapPlacementReplicaScaleState`

It intentionally does not know Scene, gameplay, renderer, physics, editor viewport, Host, or provider implementations. Producers and consumers exchange these components directly; implementation-specific rebuild behavior remains with the owning subsystem.
