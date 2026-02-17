# newengine-math

![Rust](https://img.shields.io/badge/rust-2021-000000?logo=rust&logoColor=white)
![Status](https://img.shields.io/badge/status-active-2ea44f)
![Determinism](https://img.shields.io/badge/determinism-first-6f42c1)
![Plugins](https://img.shields.io/badge/plugins-registry--driven-blue)
![Collections](https://img.shields.io/badge/collections-policies-orange)
![Unsafe](https://img.shields.io/badge/unsafe-forbid-8b0000)

`newengine-math` — фундаментальный крейт NewEngine, который **аккумулирует всю математику** и **контракты** вокруг неё:

- единая точка доступа к математическим типам и операциям
- динамический **реестр математических функций** (переопределяемый плагинами)
- централизованные **политики коллекций** (детерминизм / безопасность / стабильный порядок)
- единый подход к упаковке данных для GPU (например, матрицы)

> Принцип: *«Математика — это контракт»*.  
> Другие крейты движка не должны тянуть `glam/hashbrown/slotmap/fxhash` напрямую — только через `newengine-math`.

---

## Быстрый старт

### Добавить зависимость

```toml
[dependencies]
newengine-math = { path = "../crates/newengine-math" }
```

### Подключить прелюдии

```rust
use newengine_math::prelude::*;
use newengine_math::collections_prelude::*;
```

---

## Возможности

### 1) Динамический реестр математических функций

Реестр хранит реализации функций по `id` и позволяет:

- зарегистрировать функцию движком или плагином
- переопределить реализацию (последняя зарегистрированная становится активной)
- безопасно вызвать функцию по `id` с проверкой аргументов

**Важно:** один `id` — одна сигнатура.  
Если попытаться зарегистрировать тот же `id` с другой сигнатурой — вернётся `MathError::SignatureConflict`. Это защищает
контракт от неявного “ломания”.

---

### 2) Политики коллекций

Коллекции — часть “математического фундамента”, потому что они напрямую влияют на детерминизм, безопасность и
повторяемость поведения.

Доступны (через `newengine_math::collections::prelude` или `newengine_math::collections_prelude`):

- `FxHashMap / FxHashSet` — **детерминизм + скорость** (фиксированный seed)
- `SecureHashMap / SecureHashSet` — **DoS-устойчивость** для untrusted input
- `BTreeMap / BTreeSet` — **стабильный порядок итерации** (sorted by key)
- slotmap (`SlotMap`, `SecondaryMap`, `Key`, `new_key_type`) — стабильные generational keys

---

### 3) GPU-утилиты

`mat4_to_cols_bytes(m: Mat4) -> [u8; 64]` — единый, централизованный packing матриц в little-endian column-major, чтобы
исключить “дрейф” форматов между модулями рендера/шейдеров.

---

## Использование

### Регистрация builtins (ядро движка)

Обычно вызывается один раз на старте (до загрузки плагинов):

```rust
use newengine_math::{MathRegistry, register_engine_builtins};

fn init_math() {
    let reg = MathRegistry::global();
    register_engine_builtins(reg).expect("math builtins must register");
}
```

---

### Регистрация функции через `ne_math_fn!`

Макрос генерирует `DynMathFn`-обёртку, которая:

- хранит `Signature` в статике
- проверяет число/типы аргументов
- возвращает `MathError::InvalidArgs` при несоответствии

```rust
use std::sync::Arc;
use newengine_math::{ne_math_fn, DynMathFn, MathRegistry, MathValue, ProviderId};
use newengine_math::prelude::*;

ne_math_fn!(
    Vec3DotExt,
    "my.plugin.math.vec3.dot_ext.v1",
    [Vec3, Vec3] => F32,
    |a: Vec3, b: Vec3| {
        a.dot(b)
    }
);

fn register_plugin_math() {
    let reg = MathRegistry::global();
    let provider: ProviderId = Arc::<str>::from("my.plugin");

    reg.register(provider, Arc::new(Vec3DotExt) as Arc<dyn DynMathFn>)
        .expect("register must succeed");
}
```

---

### Вызов функции по `id`

```rust
use newengine_math::{MathRegistry, MathValue};
use newengine_math::prelude::*;

fn sample_call() {
    let reg = MathRegistry::global();

    let a = Vec3::new(1.0, 2.0, 3.0);
    let b = Vec3::new(4.0, 5.0, 6.0);

    let out = reg.call(
        "newengine.math.vec3.dot.v1",
        &[MathValue::Vec3(a), MathValue::Vec3(b)]
    ).unwrap();

    match out {
        MathValue::F32(v) => {
            // v == a.dot(b)
        }
        _ => unreachable!(),
    }
}
```

---

### Коллекции: какую карту брать и когда

```rust
use newengine_math::collections_prelude::*;

// Deterministic, fast
let mut det: FxHashMap<u32, u32> = FxHashMap::default ();

// Secure (randomized) for network/json/modding
let mut sec: SecureHashMap<String, String> = SecureHashMap::default ();

// Stable iteration order
let mut stable: BTreeMap<u32, u32> = BTreeMap::new();
```

---

## Инварианты и правила

### Контракт реестра

- `id` идентифицирует *семантику и ABI-сигнатуру* функции.
- **Один `id` — одна `Signature`.**
- Переопределение допустимо только при совпадающей сигнатуре.

### Слои зависимостей

- Другие крейты **не импортируют** `hashbrown/slotmap/fxhash/glam` напрямую.
- Все такие зависимости должны идти через `newengine-math` (типовая инициализация + политика выбора).

---

## Фичи

- `backend-glam` — временный math backend (re-export `glam` типов)
- `collections` — включает политики коллекций и re-export контейнеров
- `serde` — `serde` поддержка (если включена и backend поддерживает)

Пример:

```toml
newengine-math = { path = "../crates/newengine-math", default-features = false, features = ["backend-glam", "collections"] }
```

---

## Тесты

Крейт содержит юнит-тесты на:

- детерминизм `FxHashMap` при одинаковом порядке вставки
- работоспособность `SecureHashMap`
- стабильный порядок итерации `BTreeMap`
- регистрацию/переопределение функций
- конфликт сигнатур (`SignatureConflict`)
- `InvalidArgs` с `arg_index`

Запуск:

```bash
cargo test -p newengine-math
```

---

## Roadmap

- Постепенное уменьшение роли backend’а (`glam`) и переход на полностью движковый math backend.
- Расширение встроенных функций (intersection/geometry, transforms, noise, curves).
- Примеры интеграции для плагинов (runtime/importer/editor) и best practices по контрактам `id`.
