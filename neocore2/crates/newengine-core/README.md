# newengine-core

Core engine lifecycle, modules, typed resources, startup graph, declarative service validation, and degradation policy.

## Architecture notes

This crate is part of the North Star Engine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.
