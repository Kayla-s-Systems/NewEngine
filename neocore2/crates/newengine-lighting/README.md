# newengine-lighting

Lighting-domain data structures and helpers shared by render extraction and feature packs.

## Architecture notes

This crate is part of the CoreEngine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.
