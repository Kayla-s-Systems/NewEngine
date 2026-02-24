# 🧭 NewEngine — Архитектурная Доктрина v2.1

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

## Правило версионирования

* Любой breaking change → новая версия ABI
* Старые версии не модифицируются
* Поддержка capability negotiation

---

# 3️⃣ Capability-Driven Архитектура

## Принцип

Зависимость от capability, а не от plugin id.

## Инварианты

* Host строит capability graph
* Проверка конфликтов и циклов
* Deterministic выбор провайдера
* Required / Optional / Critical уровни

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
* Replay-система
* Hash состояния per tick

---

# 6️⃣ Единый источник математики — newengine-math

## Инвариант

* Один Vec3
* Один Mat4
* Один Quat
* Одна система контейнеров

## Запрещено

* glam
* nalgebra
* cgmath
* Локальные math-типы

---

# 7️⃣ Изоляция слоёв

Core не знает:

* Render API
* ECS storage
* Asset layout
* UI детали
* Physics детали
* Platform API

Controllers:

* Не знают рендер
* Не лезут в storage
* Работают через capability

---

# 8️⃣ Replace by Abstraction

Любой компонент заменяем через capability.

* Renderer → Vulkan / DX / Software
* AssetSource → Pak / FS / Network
* Physics → Jolt / Custom
* Audio → любой backend

---

# 9️⃣ Fail-Fast и диагностика

## Требования

* Типизированные ошибки
* Структурированный лог
* Контекстные сообщения
* Crash reports
* Correlation IDs

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

Все *-api-host вынесены за пределы ядра.

---

# 1️⃣1️⃣ Деградация вместо смерти

Отсутствие capability → управляемая деградация.

Уровни:

* Critical
* Required
* Optional

---

# 1️⃣2️⃣ Data-Oriented First

* Компактный layout
* Минимум аллокаций
* Нет аллокаций в hot path
* Пакетная обработка
* Cache-friendly структуры

---

# 1️⃣3️⃣ Thread Model

* Явная Job System
* Message passing
* Work stealing
* Минимум блокировок

Запрещено:

* Arc<Mutex<...>> в gameplay

---

# 1️⃣4️⃣ Memory Discipline

* Пулы
* Arena
* Frame allocator
* Budget-based allocation
* Leak detection в dev

---

# 1️⃣5️⃣ Instrumentation First

Всё измеряется:

* CPU профилирование
* GPU профилирование
* IO статистика
* VFS статистика
* Streaming статистика
* Allocation tracing

---

# 1️⃣6️⃣ Handle-Based Runtime

Передаются только ID и Handle:

* EntityId
* AssetId
* ResourceHandle
* ComponentId

Никаких прямых ссылок на структуры.

---

# 1️⃣7️⃣ Serialization Discipline

* Версионирование форматов
* Backward compatibility
* Deterministic serialization
* Нет runtime reflection
* Явные схемы

---

# 1️⃣8️⃣ Platform Isolation

* Platform слой — отдельный адаптер
* Нет WinAPI/LinuxAPI в Core
* Нет прямых вызовов GPU API в Core

---

# 1️⃣9️⃣ Editor ≠ Runtime

* Editor — плагин
* Runtime — минимален
* Инструменты используют те же capability

---

# 2️⃣0️⃣ Crash Containment

Если плагин падает:

* Host изолирует его
* Сохраняется диагностика
* Остальная система продолжает работу

---

# 2️⃣1️⃣ Zero Hidden Magic

Нет:

* auto-discovery без регистрации
* скрытых глобалов
* implicit behaviour
* side effects без контракта

---

# 2️⃣2️⃣ Refactor Protocol

Любое изменение проходит:

1. Проверку ABI
2. Проверку capability graph
3. Проверку детерминизма
4. Проверку деградации
5. Проверку производительности
6. Проверку CI quality gates

---

# 2️⃣3️⃣ Content Pipeline — First-Class Citizen

* Import → Cook → Pack → Patch
* Asset Build Manifest
* Хэши входов и выходов
* Версии инструментов фиксируются
* Deterministic build

---

# 2️⃣4️⃣ Layered VFS

* Pak
* Loose
* Patch
* Network
* Dev override

Deterministic priority.

---

# 2️⃣5️⃣ Streaming Architecture

* Асинхронная загрузка
* Placeholder ресурсы
* LOD деградация
* Background IO
* Budget-based eviction

---

# 2️⃣6️⃣ Save/Load и Replay

* Input recording
* Deterministic replay
* Snapshot версии
* State hash validation

---

# 2️⃣7️⃣ Security Boundary

* Trusted / Untrusted разграничение
* Sandbox политики
* Size/time limits
* Валидация паков и ассетов

---

# 2️⃣8️⃣ Quality Gates (CI/CD)

* ABI stability check
* Capability validation
* Determinism tests
* Performance regression tests
* Lint и форматирование

---

# 2️⃣9️⃣ Performance Budgets

Каждая система имеет SLA:

* CPU ms/tick
* GPU ms/frame
* IO throughput
* Allocations/frame

---

# 3️⃣0️⃣ Debuggability by Design

* Structured tracing
* Frame reports
* Crash dumps
* Telemetry snapshots

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
* Есть content pipeline
* Есть replay
* Есть quality gates

---

# 🧩 Финальная формула

NewEngine — это deterministic, data-oriented, capability-driven Host
с изолированными доменами, строгой памятью,
измеряемой производительностью,
управляемой деградацией,
заменяемыми слоями,
контрактной безопасностью
и полноценным produ
