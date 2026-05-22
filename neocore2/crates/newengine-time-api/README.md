# newengine-time-api

Stable DTOs and method constants for the `engine.time` runtime gateway.

`engine.time` owns frame time, simulation/fixed tick time, game clock, replay
clock, scheduler time, pause and time scale. Runtime domains consume
`TimeSnapshotV1`; they do not call `Instant::now()` independently.
