# newengine-camera

Camera domain types and camera-state helpers shared by gameplay and runtime camera systems.

## Architecture notes

This crate is part of the CoreEngine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.
