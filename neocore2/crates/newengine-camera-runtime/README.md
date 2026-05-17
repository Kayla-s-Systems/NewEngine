# newengine-camera-runtime

Runtime camera manager and blend-runner logic. Produces camera snapshots for downstream systems without giving renderer code ownership of gameplay camera policy.

## Architecture notes

This crate is part of the CoreEngine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.
