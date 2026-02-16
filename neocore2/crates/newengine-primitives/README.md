# newengine-primitives

Примитивы: меши/вершины/реестр, builtins (cube/plane) и компонент Primitive.

## Роль в архитектуре

- **Слой:** `crates/newengine-primitives`
- **Назначение:** Примитивы: меши/вершины/реестр, builtins (cube/plane) и компонент Primitive.

## Инварианты

- Реестр примитивов расширяемый; builtins — минимальный bootstrap для редактора/тестов.

## Публичный API

- Смотри `src/lib.rs` и модульные реэкспорты.
- Для контрактов/ABI: фиксируйте изменения через версионирование (semver) и миграции.

## Тестирование и профилирование

- Unit-тесты: `cargo test -p newengine-primitives`
- (Рекомендуется) bench/criterion для hot path.

## Ссылки

- Архитектура workspace: `../../ARCHITECTURE.md`
