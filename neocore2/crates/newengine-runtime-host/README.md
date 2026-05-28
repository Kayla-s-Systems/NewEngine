# newengine-runtime-host

Platform-facing runtime shell and typed adapters over engine gateways.

## Architecture notes

This crate is part of the North Star Engine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.
