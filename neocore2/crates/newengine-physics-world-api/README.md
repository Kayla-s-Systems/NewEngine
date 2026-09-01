# newengine-physics-world-api

ECS-facing, backend-neutral physics/world contracts.

This crate owns shared world components and the physics-query contributor boundary used by
capability runtimes. It intentionally contains no backend implementation and does not depend on
`newengine-engine-runtime`.
