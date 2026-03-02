# editor (NewEngine)

Редактор NewEngine: UI (egui/markup), viewport, управление сценой и симуляцией.

## Поток данных (сегодня)

1. UI читает ввод/состояние и генерирует `EditorCommand`.
2. `EditorSimModule` потребляет команды, мутирует `Scene` и гоняет `SimSchedule`.
3. Рендер читает мир (read-only) и рисует в viewport RT.

## Симуляция

- `Edit`: мир редактируется, fixed_update не шагает.
- `Playing`: фиксированный шаг симуляции.
- `Paused`: step-by-step.

## Roadmap

- Undo/Redo транзакции.
- Prefab/SceneAsset сериализация.
- Параллельный task graph.
