# newengine-ecs-api

Stable DTO/service contract for `engine.ecs`.

Consumers use the gateway id `engine.ecs`; concrete providers own `ecs.api` or a vendor service id plus `ecs.backend` metadata. The gateway exposes summaries, snapshots and command envelopes instead of leaking `newengine_ecs::World` across service boundaries.
