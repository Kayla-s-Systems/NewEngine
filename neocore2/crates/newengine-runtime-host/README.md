# newengine-runtime-host

Host-side runtime bootstrap слой NewEngine.

## Ответственность

- Сборка `Engine` как host/orchestrator, без editor-specific UI логики.
- Подключение platform runtime как first-class runtime unit.
- Подключение render backend runtime как first-class runtime unit.
- Общие host bootstrap utilities: ранние asset roots, window icon, log sharding.

## Не ответственность

- Не содержит editor panels, gizmos, scene tools и прочий editor UX.
- Не содержит gameplay logic.
- Не знает о конкретном app profile сверх переданных параметров.

## Зачем

Чтобы `apps/editor` перестал быть местом, где одновременно живут:

- host entry point,
- runtime bootstrap,
- editor layer.

После этого `apps/editor` становится thin profile entry point поверх общего runtime host слоя.
