# newengine-editor-runtime

Editor composition слой поверх `newengine-runtime-host`.

## Responsibility

- Собирает editor-specific runtime composition.
- Регистрирует editor render/controller modules.
- Поднимает editor UI build и scene tooling.
- Не содержит platform runtime host implementation.
- Не содержит host bootstrap / asset bootstrap дубликатов.

## Why

Чтобы `apps/editor` был thin launcher, а не hybrid-местом, где одновременно живут host bootstrap, platform runtime и
editor tools.
