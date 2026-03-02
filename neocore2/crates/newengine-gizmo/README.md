# newengine-gizmo

Gizmo-логика для редактора: трансляция/вращение/масштаб в screen-space, pick/drag и математика проекций.

## Ответственность

- Типы и режимы gizmo (`GizmoMode`, `GizmoAxis`, `GizmoSpace`).
- Математика для project/unproject, screen-ray, plane-basis, angle-on-plane.
- (feature `egui`) `EguiGizmo` — контроллер взаимодействия и отрисовка оверлея через `egui::Painter`.

## Не ответственность

- Не знает про ECS/Scene/Transform и не мутирует `World`.
- Не зависит от конкретного render backend’а (Vulkan/D3D).
- Не выполняет undo/redo (это `newengine-editor-core`).

## Features

- `egui` — включает egui-интеграцию и оверлей-рендер.

## Инварианты

- Взаимодействие должно быть устойчивым: drag не должен «прыгать» при смене масштаба/окна.
- Все вычисления должны быть детерминированны при одинаковом вводе.

## Ссылки

- `../../ARCHITECTURE.md`
