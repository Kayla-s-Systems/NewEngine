# newengine-world-api

`engine.world` is the living runtime world contract.

`engine.scene` owns authored structure: scene graph, archetype graph, prefab/archetype instances and placement declarations.
`engine.world` owns the running instance: deterministic boot, active cells, partition state, world snapshot and runtime state coordination.

ECS remains storage behind coarse commands and snapshots. This API exposes opaque entity handles only.
