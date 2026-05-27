# UI Node Gateway Refactor — 2026-05-27

> [!NOTE] REQUEST NOTE — текущее положение дел
> **У нас сейчас:** `engine.ui` остаётся единственной backend-точкой для UI presentation. Aurelia является provider implementation за `engine.ui`, а Assets Catalog UI projection переведён в retained UI node поверх `engine.assets`, без собственного backend gateway/domain.
> **Было бы здорово:** следующим pass добавить conformance-тесты для `UiSurfaceNode` ordering, missing-route diagnostics и UI provider absence.
> **Technical details (EN):** `newengine-assets-catalog-ui-runtime`, `UiSurfaceNode`, `UI_SERVICE_METHOD_SURFACE_NODE_V1`, `engine.assets`, `engine.ui`.

## Invariant

```text
engine.ui is the API point.
engine.ui.aurelia is one implementation.
Assets Catalog UI projection is a UI node composition, not a backend domain.
Renderers do not draw UI fallback overlays.
Missing UI provider emits diagnostics and skips UI drawing.
ESC menu is a top modal UI surface above all other nodes.
```
