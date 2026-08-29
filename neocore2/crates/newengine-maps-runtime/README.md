# newengine-maps-runtime

Provider-neutral semantic service implementation for the host-owned `engine.assets.maps` gateway.

This crate builds the `maps.api` service but does not register a route with the Host. The standalone `PluginsSrc/MapsRuntime` artifact owns provider identity, capability metadata, priority and service registration, so another plugin can replace it without rebuilding a runtime profile.

The provider resolves canonical `.ymap` map declarations into `MapIndexV1` and
`MapCellV1` DTOs. It never mutates ECS/world state. `engine.scene` / `engine.world`
apply stages own instantiation.

Canonical selectors:

```text
maps/world.ymap@map
maps/world.ymap@cell/0/0
maps/world.ymap@cell/-1/2
```

Placements reference `.ytyp@entry` definitions only. Model/material/texture,
physics, AI and render semantics remain in their owning gateways.
