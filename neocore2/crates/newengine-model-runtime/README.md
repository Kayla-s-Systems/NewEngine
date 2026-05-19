# newengine-model-runtime

Gateway-backed model construction runtime.

`ModelAssetAdapter` is pure: it receives an `AssetServiceClient` through
`with_client(...)` and never calls `default_host_api()` itself. Product/profile
composition is responsible for wiring the active host/AssetManager service and
registering the `engine.model` gateway.
