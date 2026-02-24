# 🧭 NewEngine — ECS / Entity / Transform / Camera / Scene

## AAA-уровень изоляции и мягкой деградации

---

# 0️⃣ Главный архитектурный инвариант

> Ни один доменный модуль не зависит от реализации другого.
> Всё взаимодействие происходит через Engine и Capability-контракты.

* Engine — единственная точка связывания.
* Домены регистрируются как сервисы.
* Доступ — только через `ServiceRegistry`.
* При отсутствии сервиса — **деградация, не смерть**.

---

# 1️⃣ Разделение доменов

## 🔷 ECS — `ecs.world.v1`

Отвечает за:

* EntityId
* Компоненты
* Системы
* Schedule

Не отвечает за:

* Scene graph
* Transform математику
* Камеру
* Рендер

---

## 🔷 Scene — `scene.graph.v1`

Отвечает за:

* Семантическую структуру
* Иерархию узлов
* Слои / уровни
* Связь Node ↔ Entity (опционально)

Не отвечает за:

* Хранение трансформов
* ECS storage
* Рендер

---

## 🔷 Transform — `transform.store.v1`

Отвечает за:

* Local TRS
* World матрицы
* Dirty propagation

Не отвечает за:

* Иерархию сцены
* Создание сущностей
* Камеру

---

## 🔷 Camera — `camera.rig.v1`

Отвечает за:

* Rig state (yaw, pitch, distance, fov)
* View/Projection
* Контроллеры (orbit/free/follow)
* Smooth damp

Не отвечает за:

* Хранение transform
* ECS storage
* Scene graph

Контроллеры обязаны работать через сервисы, а не напрямую лезть в ECS.

---

# 2️⃣ Engine как единственный медиатор

```text
Controller
    ↓
Engine.get(transform.store.v1)
    ↓
Transform API
```

```text
Camera
    ↓
Engine.get(transform.store.v1)
Engine.get(input.service.v1)
    ↓
View/Proj
```

Ни одного прямого импорта `scene → transform`
Ни одного прямого импорта `camera → ecs`

---

# 3️⃣ Entity — только идентификатор

```text
EntityId = (index + generation)
```

* Никаких методов `entity.get_transform()`
* Никаких скрытых связей
* Entity — чистый ключ

Связи хранятся в сервисах.

---

# 4️⃣ Transform без знания Scene

## Истина позы хранится только в TransformStore

```text
create_handle() → TransformHandle
set_local()
get_world()
propagate()
```

### Иерархия НЕ хранится внутри Transform

Transform получает parent-child через:

```text
hierarchy.provider.v1 (optional)
```

Если сервиса нет:

* world = local
* propagation отключается
* логируется деградация

Система продолжает работать.

---

# 5️⃣ Camera как чистый solver

Camera не владеет Transform.

Она:

1. Читает world transform
2. Обновляет rig state
3. Возвращает intent
4. Engine применяет intent

---

## 🔁 Важно для стабильности RMB

В одном кадре:

* Только один владелец записи Transform
* Остальные — только чтение
* Порядок строго детерминирован

---

# 6️⃣ Frame Pipeline (детерминированный)

1. InputService → snapshot
2. Controllers → обновление rig state
3. Apply intents → запись transform
4. Scene updates
5. Transform propagation
6. Render extraction
7. Render

Никаких скрытых перезаписей.

---

# 7️⃣ Capability-матрица зависимостей

| Домен     | Обязательные | Опциональные |
|-----------|--------------|--------------|
| ECS       | —            | Scene        |
| Scene     | —            | ECS          |
| Transform | —            | Hierarchy    |
| Camera    | Transform    | Input        |
| Renderer  | Transform    | Scene        |

Отсутствие опционального сервиса → мягкая деградация.

---

# 8️⃣ Мягкая деградация (НЕ СМЕРТЬ)

Каждый запрос:

```rust
Option<ServiceHandle>
```

или

```rust
Result<ServiceHandle, MissingService>
```

И логируется в DiagBus.

---

### Примеры

* Нет Transform → Scene работает без world pose
* Нет Hierarchy → world = local
* Нет Camera → fallback camera
* Нет Scene → ECS работает автономно

---

# 9️⃣ Null Service Pattern

Engine регистрирует:

* Real service
  или
* Null implementation

Null сервис:

* Не делает ничего
* Возвращает identity
* Логирует один раз

---

# 🔟 Строгие запреты

Controllers не должны:

* Лезть напрямую в ECS storage
* Знать реализацию Scene
* Хардкодить input внутри себя

---

# 1️⃣1️⃣ Расширяемость уровня AAA

Эта схема позволяет:

* Запускать ECS без сцены (сервер)
* Запускать сцену без ECS (визуализация)
* Подменять Transform реализацию
* Подключать альтернативную камеру
* Убирать рендер из ядра

---

# 1️⃣2️⃣ Финальная модель

```text
Engine
 ├── ServiceRegistry
 ├── Scheduler
 ├── DiagBus
 └── CapabilityMatrix
        ↓
    ECS
    Scene
    Transform
    Camera
```

Ни один из них не знает о реализации другого.
Все общаются только через контракт.

---

# Итог

✔ Полная изоляция доменов
✔ Capability-driven взаимодействие
✔ Детерминированный pipeline
✔ Null-service деградация
✔ AAA-архитектурная масштабируемость
✔ Engine остаётся чистым Host
