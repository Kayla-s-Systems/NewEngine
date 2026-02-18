# newengine-assets

Engine-side **AssetManager service contract** and a small, protocol-first client.

This crate exists so any engine subsystem (renderer, editor core, UI, tools, etc.) can interact with the
AssetManager runtime plugin (DLL) without depending on UI crates or on a concrete AssetManager implementation.

## What you get

- `AssetServiceClient`: calls the AssetManager service through `HostApiV1::call_service_v1`.
- `AssetService` / `AssetAccess`: minimal traits for systems that want to stay mockable/testable.
- Contract constants: service id and canonical method names.

The concrete AssetManager implementation lives in the `newengine-AssetManager` plugin.
