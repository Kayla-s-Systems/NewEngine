# NewEngine Math & Collections Policy

## Цель

`newengine-math` является единой точкой входа для математических типов, математических операций и низкоуровневых коллекций движка.

## Правило для движка

Hot-path код движка использует `newengine-math` напрямую:

```rust
use newengine_math::prelude::*;
use newengine_math::collections_prelude::*;
```

Запрещено тянуть в доменные крейты и плагины прямые зависимости/импорты на:

- `hashbrown::*`
- `fxhash::*`
- `ahash::*`
- `std::collections::*` в runtime-логике движка/плагинов
- сторонние math crates напрямую

Если нужна коллекция, она должна быть выбрана через engine policy:

- `NeHashMap`, `NeHashSet` — внутренние быстрые контейнеры.
- `NeSecureHashMap`, `NeSecureHashSet` — данные из сети, файлов, JSON, модов, пользователя.
- `NeBTreeMap`, `NeBTreeSet` — стабильный порядок итерации/сериализации.
- `NeVecDeque` — очереди/LRU/FIFO.

## Правило для плагинов

Плагин не выбирает hasher/collection backend сам. Он использует только `newengine_math::collections_prelude`.

Для математических расчётов есть два пути:

1. **Hot-path / per-frame / render / physics:** прямой вызов типов и функций из `newengine-math`.
2. **Расширяемые/редакторские/межплагиновые операции:** вызов сервиса `newengine.math.service.v1` из `MathPlugin`.

## Почему math не должен быть только DLL

Полностью вынести всю математику в DLL нельзя без деградации архитектуры:

- per-frame `Vec3::dot`, `Mat4 * Vec4`, normalize, camera/frustum/renderer должны инлайниться;
- ABI/JSON/service-call на каждый скалярный расчёт убьёт hot-path;
- deterministic foundation должен быть доступен на этапе компиляции для engine crates.

Правильная модель: `newengine-math` — фундаментальный crate, `MathPlugin` — сервисный/расширяемый provider поверх того же контракта.

## CI-запреты

Минимальный grep/audit gate:

```bash
rg "hashbrown|fxhash|ahash|std::collections" NewEngine/neocore2/crates Plugins -g '*.rs' -g 'Cargo.toml'
```

Допустимые исключения:

- внутренняя реализация `crates/newengine-math/src/collections*`;
- внешние библиотечные примеры/тестовые fixtures;
- код, где `std::collections` нужен только для FFI/adapters и явно обёрнут policy-типом.
