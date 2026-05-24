# newengine-jobs-api

Stable API contract for `engine.jobs`.

`engine.jobs` is the runtime job/task control surface. Long-running asset loads, shader compilation, package mounts, scene spawn, script threads, flow execution and AI planning batches should publish `JobId` lifecycle/progress events through the engine bus and support cooperative pause/resume/cancel checkpoints where safe.
