# newengine-plugin-api

Stable plugin ABI definitions: plugin roots, modules, host API, services, descriptors, and capabilities.

## Architecture notes

This crate is part of the CoreEngine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.
