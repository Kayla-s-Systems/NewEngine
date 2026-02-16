# newengine-ecs

Детерминированный ECS для NewEngine: сущности с генерациями, компонентные хранилища, запросы и командный буфер.

## Роль в архитектуре

- **Слой:** `crates/newengine-ecs`
- **Назначение:** минимальный, предсказуемый ECS-ядро без скрытых аллокаций и без неявных сайд‑эффектов.

## Инварианты и правила использования

1) **EntityId — generational key.**
   - Нельзя использовать как persistent id в сценах/ассетах/сохранениях.
   - Для сохранений нужен отдельный стабильный идентификатор (GUID/Path/AssetId и т.п.).

2) **Структурные изменения — через `Commands`.**
   - В системах избегаем прямых `spawn/despawn/insert/remove` на `World`, чтобы не ловить borrow hazards.
   - `Commands` применяются детерминированно (двухфазный коммит: сначала spawn, затем остальное в исходном порядке).

3) **Change tracking (`added_tick` / `changed_tick`) привязан к `World::tick`.**
   - Tick должен быть монотонным и управляться runtime’ом (frame index или fixed tick).
   - `get_mut()` **не** помечает `changed` (намеренно).
   - Для мутаций используйте `get_mut_tracked()` или `query_mut_tracked()`.

4) **Детерминизм:**
   - Итерация по компонентам использует `slotmap::SecondaryMap` (стабильная логика при одинаковом порядке записей).
   - Порядок запуска систем определяется `Schedule` по ключу `(Stage, order, insertion_seq)`.

## Быстрый пример

```rust
use newengine_ecs::{World, Commands, Schedule, Stage, System, FrameCtx};

#[derive(Clone, Copy)]
struct Pos(f32);

struct MoveSys;
impl System for MoveSys {
   fn name(&self) -> &'static str { "move" }
   fn stage(&self) -> Stage { Stage::FixedSim }
   fn run(&mut self, world: &mut World, _cmd: &mut Commands, frame: FrameCtx) {
      if let Some(it) = world.query_mut_tracked::<Pos>() {
         for (_id, p) in it {
            p.0 += frame.fixed_dt;
         }
      }
   }
}
```

## Публичный API

- `World` — хранит сущности, компоненты, ресурсы, tick.
- `Query / Query2 / QueryMut / QueryMutTracked` — итераторы без аллокаций.
- `Commands` — детерминированный командный буфер.
- `Events<T>` — double-buffer очередь событий (видимость строго через `swap()`).
- `Schedule / Stage / System / FrameCtx` — минимальный детерминированный запуск систем.

## Тестирование и профилирование

- Unit-тесты: `cargo test -p newengine-ecs`
- Рекомендуется добавить criterion‑бенчи для hot path (`insert/get/query/query2/commands.apply`).

## Ссылки

- Архитектура workspace: `../../ARCHITECTURE.md`
