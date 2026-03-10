# editor (NewEngine)

`apps/editor` больше не является местом, где живут host bootstrap, platform runtime и editor tools одновременно.

Теперь это thin launcher editor-profile.

## Responsibility

- Загружает startup config.
- Поднимает общий `newengine-runtime-host`.
- Подключает `newengine-editor-runtime` как editor composition layer.
- Запускает platform runtime.

## Non-responsibility

- Не содержит host/platform runtime implementation.
- Не содержит render runtime implementation.
- Не дублирует asset bootstrap utilities.

## Architecture

- `newengine-runtime-host` — общий host foundation.
- `newengine-editor-runtime` — editor-specific composition.
- `apps/editor` — thin launcher поверх этих слоёв.
