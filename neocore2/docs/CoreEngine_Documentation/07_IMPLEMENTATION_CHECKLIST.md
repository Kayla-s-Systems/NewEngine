# Provider / Adapter Implementation Checklist

## Universal rules

- [ ] Provider identity comes from ABI descriptor/signature, not filename.
- [ ] Provider declares `ServiceV1` in `PluginDescriptor`.
- [ ] Provider declares a backend capability in `PluginDescriptor`.
- [ ] Backend capability metadata contains `service_kind`.
- [ ] Backend capability metadata contains `engine_gateway`.
- [ ] Backend capability metadata contains `contract` when multiple `ServiceV1` entries exist.
- [ ] Backend capability metadata contains deterministic `backend_priority`.
- [ ] Consumers call `engine.*` gateway ids, not provider service ids.
- [ ] No hidden in-process fallback backend.
- [ ] Null backend is a real service provider plugin.
- [ ] Runtime imports API crates/adapters only.
- [ ] Backend receives DTO packets, not engine internals.
- [ ] Backend shutdown is idempotent.

## Gateway routing checklist

- [ ] Routing is descriptor-driven.
- [ ] Unknown `service_kind` logs a warning and ignores the route.
- [ ] Invalid `engine_gateway` logs a warning and ignores the route.
- [ ] Multiple providers are selected by priority and deterministic tie-breakers.
- [ ] `list_services()` includes active engine gateways for diagnostics.
- [ ] `describe_service(engine.*)` returns the active provider description.
- [ ] `call_service_v1(engine.*, method, payload)` routes to the active provider service.

## Current status

- [x] Assets use `engine.assets`.
- [x] Render uses `engine.render`.
- [x] Physics uses `engine.physics`.
- [x] Input is gateway-routable through `engine.input`.
- [x] ECS world summary/snapshot uses `engine.ecs`.
- [x] Entity identity/lifecycle uses `engine.entity`.
- [x] Platform window snapshot uses `engine.platform`.
- [ ] Provider conformance tests.
- [ ] Zero-warning CI.
