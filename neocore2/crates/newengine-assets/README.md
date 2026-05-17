# newengine-assets

Asset domain helpers and runtime-facing asset abstractions. Asset consumers should call `engine.assets` through the host client instead of binding to a concrete provider service id.

## Architecture notes

This crate is part of the CoreEngine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.
