# newengine-scene

Scene-домен NewEngine: persistent identity, семантические компоненты и (опционально) рантайм-обвязка поверх ECS.

## Ответственность

- Scene contracts (по умолчанию):
    - `EntityGuid`, `Name`, `PropertyBag`
    - маркеры ролей: `SceneRoot`, `ActiveCamera`
    - `SceneSettings` (оси/масштаб единиц)

- Runtime (feature `runtime`):
    - `Scene` = owned `World` + `SceneSettings`
    - инварианты (`SceneState`, reconcile unique markers)
    - derived state pipeline: `update_scene_world`

## Не ответственность

- Не содержит renderer/viewport/UI.
- Не делает «магических» спавнов по умолчанию: bootstrap (root/camera) — задача верхних слоёв.

## Features

- `runtime` — включает интеграцию с ECS + Transform + Bounds.

## Инварианты

- `EntityGuid` — единственный persistent id для сцены.
- Derived state обновляется централизованно (`update_scene_world`), а не распределённо.

## Ссылки

- `../../ARCHITECTURE.md`
