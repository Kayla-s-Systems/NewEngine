# newengine-host-capabilities-runtime

Thin composition/service layer for Host PreInit capabilities.

The crate **does not probe hardware itself**. `HostCapabilityFactory` composes single-purpose leaf crates for environment, platform primitives, CPU, memory, GPU, storage, displays, and input, then delegates immutable provider-selection policy to `newengine-host-capability-policy`.

This is the intended UNIX/factory boundary: leaf crates do one thing and never register Host services; this crate is the composition root that builds the stable `HostPreInitSnapshot` contract and exposes the native provider service.
