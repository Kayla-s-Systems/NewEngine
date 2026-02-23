# 🧭 NewEngine — Архитектурная Доктрина v2

## Production-Grade / AAA Standard

---

# 1️⃣ Engine as Host. Service as Plugin.

## Принцип

Ядро — оркестратор жизненного цикла.
Вся функциональность реализуется через плагины.

## Требования

* Core не знает конкретных типов сервисов
* Нет прямых зависимостей `core → доменный модуль`
* Взаимодействие только через ABI и capability
* Core компилируется без любых конкретных плагинов

## Архитектурный инвариант

Core — слепой Host.
Домены — изолированные расширения.

---

# 2️⃣ ABI — это закон

## Принцип

`newengine-plugin-api` — чистый контракт.

## В ABI допустимо

* PluginId
* PluginKind
* Version
* CapabilityDesc[]
* Экспортируемые точки входа

## В ABI запрещено

* Типы ассетов
* Структуры сервисов
* Доменные enum
* Match по типам
* Знание конкретных плагинов

---

# 3️⃣ Capability-Driven Архитектура

## Принцип

Зависимость от capability, а не от plugin id.

❌ Неправильно

```rust
if plugin.id == "newengine.assets"
```

✅ Правильно

```rust
if plugin.capabilities.contains("asset.registry.v1")
```

## Инвариант

Любая функциональность определяется capability.

---

# 4️⃣ Конфиг — источник истины

## Принцип

Поведение задаётся конфигом или capability.

## Запрещено

* Захардкоженные пути
* Вшитые расширения
* Скрытые fallback без логирования

## Правильно

* Явная регистрация VFS source
* Приоритет слоёв задаётся конфигом
* Importer объявляет форматы

---

# 5️⃣ Determinism First

## Принцип

Одинаковый input → одинаковый результат.

## Требования

* Deterministic контейнеры
* Сортировка регистрации
* Фиксированный порядок инициализации
* Отсутствие недетерминированной итерации

---

# 6️⃣ Единый источник математики — newengine-math

## Принцип

Вся математика используется исключительно из `newengine-math`.

## Запрещено

* glam
* nalgebra
* cgmath
* Локальные math-типы

## Инвариант

Один Vec3, один Mat4, один Quat во всей экосистеме.

---

# 7️⃣ Изоляция слоёв

## Core не знает

* Render API
* ECS storage
* Asset layout
* UI детали
* Physics детали

## Controllers

* Не знают рендер
* Не лезут в storage
* Работают через capability

---

# 8️⃣ Replace by Abstraction

Любой компонент заменяем:

* Renderer → Vulkan / DX / Software
* AssetSource → Pak / FS / Network
* Physics → Jolt / Custom

---

# 9️⃣ Fail-Fast и диагностика

## Принцип

Ошибки не должны быть тихими.

## Требования

* Контекстные ошибки
* Структурированный лог
* Типизированные error types

## Запрещено

* unwrap()
* expect без контекста
* swallow error

---

# 🔟 Минимальное ядро

Core содержит только:

* Plugin Manager
* Capability Registry
* Service Router
* Lifecycle
* Startup Config
* Host API

---

# 1️⃣1️⃣ Деградация вместо смерти

## Принцип

Отсутствие capability → управляемая деградация.

## Уровни

* Critical
* Required
* Optional

Host обязан различать уровни.

---

# 1️⃣2️⃣ Data-Oriented First

## Принцип

Производительность определяется структурой данных.

## Требования

* Компактный layout
* Минимум аллокаций
* Нет аллокаций в hot path
* Пакетная обработка

---

# 1️⃣3️⃣ Thread Model

## Принцип

Потоки — часть архитектуры.

## Требования

* Явная Job System
* Message passing
* Минимум блокировок

## Запрещено

* Arc<Mutex<...>> в gameplay

---

# 1️⃣4️⃣ Memory Discipline

## Требования

* Пулы
* Arena
* Frame allocator
* Leak detection в dev

---

# 1️⃣5️⃣ Instrumentation First

## Принцип

Всё измеряется.

## Требования

* CPU/GPU профилирование
* Статистика VFS
* Статистика стриминга
* Тайминги систем

---

# 1️⃣6️⃣ Handle-Based Runtime

Передаются ID, а не структуры.

```rust
EntityId
AssetId
ResourceHandle
```

---

# 1️⃣7️⃣ Serialization Discipline

* Версионирование форматов
* Backward compatibility
* Deterministic serialization
* Нет runtime reflection

---

# 1️⃣8️⃣ Platform Isolation

Core не знает:

* Windows API
* Linux API
* Vulkan
* DirectX

---

# 1️⃣9️⃣ Editor ≠ Runtime

Editor — плагин.
Runtime — чистый.

---

# 2️⃣0️⃣ Crash Containment

Если плагин падает:

* Host изолирует
* Диагностика сохраняется
* Остальная система продолжает работу

---

# 2️⃣1️⃣ Zero Hidden Magic

Нет:

* auto-discovery без регистрации
* скрытых глобалов
* implicit behaviour

---

# 2️⃣2️⃣ Refactor Protocol

Любое изменение проходит:

1. Проверку контрактов
2. Проверку capability graph
3. Проверку детерминизма
4. Проверку деградации
5. Проверку профилирования

---

# 🧱 Критерий AAA-готовности

Система зрелая если:

* Core собирается без доменов
* Любой плагин заменяем
* Любой источник отключаем
* Всё capability-driven
* Всё детерминировано
* Всё измеряется
* Падения изолируются

---

# 🧩 Финальная формула

NewEngine — это deterministic, data-oriented, capability-driven Host
с изолированными доменами, строгой памятью,
измеряемой производительностью и заменяемыми слоями.
