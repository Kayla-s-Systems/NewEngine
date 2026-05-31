# newengine-tags-api

Stable DTO contract for `engine.tags`.

Gameplay systems communicate through declared tags instead of scattered strings
or hardcoded enum branches. The runtime may attach tag snapshots to entities,
AI agents, tasks, animation channels and authored resources, but service
boundaries expose only `EntityHandle` and DTOs.
