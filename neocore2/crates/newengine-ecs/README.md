# newengine-ecs

Entity-component storage and runtime world primitives used by engine systems.

## Architecture notes

This crate is part of the CoreEngine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.
