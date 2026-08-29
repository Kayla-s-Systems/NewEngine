# newengine-audio-world-api

Provider-neutral transient world-audio contracts shared by physics-facing contributors and the world-audio implementation.

This crate owns **types only**. It must not own Scene/ECS access, provider registration, runtime-unit factories, mixer calls, or engine-runtime composition.
