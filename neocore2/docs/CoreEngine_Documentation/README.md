# CoreEngine / NewEngine Documentation

Updated: 2026-05-17

This documentation describes the current host/plugin architecture after the Gateway Service Layer decomposition.

## Current model

```text
Engine/Core = lifecycle + scheduler + Resources + startup graph + gateway registry
Gateway     = stable engine-facing facade id, e.g. engine.render
Provider    = plugin-owned service implementation, e.g. render.api or vendor.render.api
Adapter     = typed host-side wrapper over the selected route
Feature     = explicit plugin/profile capability, not hidden engine code
```

Consumers call stable engine-owned gateways:

```text
engine.assets
engine.render
engine.physics
engine.input
engine.camera
engine.ui
engine.scene
engine.ecs
engine.entity
engine.platform
```

The host maps those gateways to active providers from plugin metadata.

## Files

1. `01_ARCHITECTURE_OVERVIEW.md`
2. `02_FUNCTIONALITY_GUIDE.md`
3. `03_PROVIDER_ADAPTER_SYSTEM_GUIDE.md`
4. `04_RENDER_PROVIDER_AUDIT.md`
5. `05_PHYSICS_PROVIDER_AUDIT.md`
6. `06_TECH_DEBT_AND_GAPS_ROADMAP.md`
7. `07_IMPLEMENTATION_CHECKLIST.md`
8. `08_SOURCE_MAP.md`
9. `09_RENDER_STRICT_SERVICE_BACKEND_REPORT.md`
10. `10_PHYSICS_REPLACEABILITY_RECORD.md`
11. `11_STATIC_SCAN_SUMMARY.md`
12. `12_NEYTD_CODEC_SPEC.md`
13. `13_NEPAK_CODEC_SPEC.md`
14. `14_RELEASE_READINESS_SNAPSHOT.md`
15. `15_GATEWAY_SERVICE_LAYER.md`, `16_ENGINE_ECS_GATEWAY.md`
16. `17_ENGINE_PLATFORM_GATEWAY.md`
17. `18_ENGINE_ENTITY_GATEWAY.md`

## Latest gateway passes

- `16_ENGINE_ECS_GATEWAY.md` — ECS service gateway boundary.
- `17_ENGINE_PLATFORM_GATEWAY.md` — platform gateway boundary and native window snapshot method.
- `18_ENGINE_ENTITY_GATEWAY.md` — entity identity/lifecycle gateway boundary.
