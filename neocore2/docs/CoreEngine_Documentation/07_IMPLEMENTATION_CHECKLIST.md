# 07 — Implementation checklist для providers/adapters

## 1. Universal rules

- [ ] Provider identity comes from ABI descriptor/signature, not filename.
- [ ] Provider declares service and capability in `PluginDescriptor`.
- [ ] No legacy aliases unless explicitly approved for a short migration branch.
- [ ] No hidden in-process fallback backend.
- [ ] Null backend is a real service provider plugin.
- [ ] Runtime imports API crates/adapters only.
- [ ] Backend receives DTO packets, not engine internals.
- [ ] Backend shutdown is idempotent.

## 2. Provider plugin checklist

- [ ] Implements `PluginModuleV3`.
- [ ] Stable plugin id/name/version.
- [ ] Correct `PluginKind::Runtime` for runtime backend.
- [ ] Declares service capability with `CapabilityKind::ServiceV1`.
- [ ] Declares domain backend capability.
- [ ] `describe_json` includes protocol/features/limits/priority.
- [ ] Does not register undeclared services.
- [ ] Unknown service methods return structured error.
- [ ] Bad payload returns structured error.

## 3. Host adapter checklist

- [ ] Client uses API crate constants only.
- [ ] Adapter hides raw service blobs from engine systems.
- [ ] Adapter maps provider errors to engine errors with context.
- [ ] Adapter validates service owner capability.
- [ ] Adapter registers typed API ref in `Resources`.
- [ ] Adapter unregisters API ref on shutdown.

## 4. Render status

- [x] Provides `render.api`.
- [x] Provides `render.backend`.
- [x] Runtime binds through `RenderServiceClient` and `ServiceBackedRenderApi`.
- [x] No in-process renderer fallback.
- [x] Null renderer is service plugin.
- [x] No backend aliases/config selector.
- [x] Descriptor/capability provider selection.
- [ ] Provider conformance tests.
- [ ] Zero-warning CI.

## 5. Physics status

- [x] `newengine-physics-api` exists.
- [x] Providers declare `physics.api`.
- [x] Providers declare `physics.backend`.
- [x] Host-side `PhysicsServiceClient` exists.
- [x] Runtime registers `PhysicsApiRef`.
- [x] Engine-runtime does not import provider implementation crates.
- [x] No `&mut World` crosses provider boundary.
- [x] Static terrain/world geometry is packetized as DTO colliders.
- [x] Descriptor-first discovery, no filename identity.
- [ ] Contact/replay conformance tests.
- [ ] Binary packet option if profiling requires it.
