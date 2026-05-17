# CoreEngine / NewEngine — документация provider architecture

Дата обновления: 2026-05-17  
Источник состояния: `NewEngineRockstar-source-20260517-085857.zip` + последний runtime log `game-ready-fps`.  
Фокус этого пакета: актуальное описание движка после перевода physics на `physics.api` service adapter plugin и формализация proprietary asset codecs `.neytd` / `.nepak`.

## Главный вывод

CoreEngine стал ближе к release-ready границе:

```text
Engine/Core = lifecycle + scheduler + Resources + service registry + plugin host
Backend     = DLL provider за стабильным service API
Adapter     = host-side typed wrapper over service protocol
Feature     = explicit plugin / profile feature pack
```

Renderer и physics теперь следуют одной модели:

```text
provider plugin
  declares <domain>.backend capability
  registers <domain>.api service
      ↓
runtime-host service client
      ↓
typed adapter registered in Resources
      ↓
engine-runtime systems talk only to API packets
```

Критический инвариант: движок не должен знать, какой конкретный physics backend исполняет физику. Он видит только `physics.api`, `physics.backend`, DTO packets и `PhysicsApiRef`.

## Что считается текущим состоянием

- Render backend: service plugin через `render.api`.
- Physics backend: service plugin через `physics.api`.
- Plugin discovery: descriptor-first через ABI probe; имя DLL не является identity и не используется для выбора backend provider.
- Physics frame boundary: `ECS -> PhysicsFrameInput -> physics.api -> PhysicsFrameOutput -> ECS`.
- Static world collision: через формальные collider DTOs (`HeightfieldColliderDto`, `MeshColliderDto`), а не backend-native handles.
- Asset runtime texture format: `.neytd`.
- Asset archive/container format: `.nepak`.

## Файлы пакета

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
