# 03 — Гайд: чистая система providers/adapters

## 1. Термины

**Provider** — plugin or feature pack that provides a declared capability/service.

**Backend provider** — plugin behind stable service API, e.g. `render.api` or `physics.api`.

**Adapter** — host-side typed wrapper over a service protocol, registered in `Resources` as a typed API ref.

## 2. Strict rules

- No backend identity by filename.
- No config aliases such as `auto`, `default`, vendor names or backend nicknames as engine selection logic.
- No hidden in-process fallback backend.
- Null backend must be a real service provider plugin.
- Runtime systems import API crates/adapters, not provider implementation crates.
- No provider backend receives `&mut World` or backend-native handles across ABI.
- Backend priority is declared metadata, not hard-coded engine knowledge.

## 3. Descriptor-first discovery

A runtime provider must self-describe through ABI metadata:

```rust
PluginDescriptor::builder(plugin_id, name, version, PluginKind::Runtime)
    .provides_service(SERVICE_ID, 1, r#"{...}"#)
    .push(
        CapabilityDesc::new("<domain>.backend", Provides, Other, 1)
            .with_json(r#"{"backend_priority":100}"#)
    )
    .build()
```

Selection is based on descriptor facts:

```text
service id + backend capability + backend_priority
```

The file path is only the thing to load. It is not the source of plugin identity.

## 4. Render reference implementation

```text
Renderer plugin
  provides service render.api
  provides capability render.backend
      ↓
RenderServiceClient
      ↓
ServiceBackedRenderApi : dyn RenderApi
      ↓
Resources.register_api(RENDER_API_ID, RenderApiRef)
      ↓
RuntimeRenderController
```

## 5. Physics implementation

```text
Physics plugin
  provides service physics.api
  provides capability physics.backend
      ↓
PhysicsServiceClient
      ↓
ServiceBackedPhysicsApi : dyn PhysicsApi
      ↓
Resources.register_api(PHYSICS_API_ID, PhysicsApiRef)
      ↓
PhysicsSyncModule
```

`PhysicsSyncModule` is the only place where ECS is translated into physics packets and output packets are applied back to ECS.

## 6. Adapter lifecycle

```text
Provider lifecycle:
  init_v3(host, config)
    register_service_v1(...)
  shutdown()
    release native/backend resources

Host adapter lifecycle:
  init(ctx)
    service client info/negotiate
    validate loaded service owner capability
    register typed API ref into Resources
  update/fixed_update
    call typed API methods
  shutdown(ctx)
    unregister typed API ref
```

## 7. Command-buffer boundary

Bad:

```text
physics.step(&mut World)
renderer.draw_scene(&SceneBridge)
```

Good:

```text
physics.step(PhysicsFrameInput) -> PhysicsFrameOutput
renderer.submit_frame(RenderFrameEnvelope) -> RenderFrameSubmitReport
```

## 8. Null providers

Null providers are not hidden compatibility shims. They are ordinary service providers with declared capabilities. This makes headless/no-op behavior testable and observable.
