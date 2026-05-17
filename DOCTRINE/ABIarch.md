# ABI Architecture Doctrine

Runtime plugin boundaries must be explicit and stable.

Rules:

- plugin identity comes from exported ABI descriptors;
- services are registered through `ServiceV1`;
- capability metadata describes provider role and gateway route;
- host code validates descriptors before binding adapters;
- engine-runtime never imports provider implementation crates.

Versioned contracts are required for long-lived plugin compatibility. Breaking changes should create new contract versions instead of silently changing existing payloads.
