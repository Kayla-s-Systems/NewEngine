# newengine-transform-api

Контракты трансформа и иерархии: компоненты, world-space helpers и service ids/vtable для рантайм-интеграции.

## Ответственность

- Data contracts:
    - `Transform` (local TRS)
    - `Parent`, `Children`, `TransformDirty` (feature `ecs`)
    - derived outputs: `GlobalTransform`, `WorldPose`
- API helpers (feature `ecs`):
    - чтение/запись world-space позы
    - `set_parent(world, child, parent)` — поддержка `Children` и dirty-mark
- Service contract:
    - `TRANSFORM_SERVICE`, `ITRANSFORM_RUNTIME_V1`
    - `runtime::TransformRuntimeVTable` + `TransformRuntimeApi`

## Не ответственность

- Не содержит алгоритм пропагации и кэширование scratch (это `newengine-transform`).
- Не зависит от `newengine-core` (реестр сервисов/модули — выше уровнем).

## Features

- `ecs` — включает компоненты и helpers, зависящие от `newengine-ecs`.

## Инварианты

- Компоненты — чистые данные, пригодные для ECS и сериализации (где уместно).
- Идентификаторы сервисов должны оставаться стабильными.

## Ссылки

- `../../ARCHITECTURE.md`
