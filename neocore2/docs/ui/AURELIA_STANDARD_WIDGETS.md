# Aurelia Standard Widgets

Aurelia renders provider-neutral UI atoms from `UiComponentNode` / `UiNodeRequest`.

Supported visual atoms today:

- `button`, `action`
- `input`
- `checkbox`, `toggle`
- `slider`
- `scroll_bar`
- `select`
- `separator`
- `list`, `tree`
- `panel`, `row`, `text`, `viewport`, `external_texture`

State is data-driven through `state_tags` and props:

```text
hovered
active
selected
focused
disabled
checked
```

Numeric controls use generic props:

```json
{
  "progress_01": 0.5,
  "value_01": 0.5,
  "offset_01": 0.0,
  "page_01": 0.25,
  "w_px": 240,
  "h_px": 120
}
```

The provider must not know products such as Asset Browser, Log Viewer, Profiler, EditorScreen, Material Browser, Memory Visualizer, Particle Editor, Symbol Tool, or Engine Registry.

Product modules publish UI nodes and interaction state; Aurelia resolves layout/input/hit-test/state and emits generic paint commands for the renderer.

The long-term renderer boundary and editor component target set are defined in:

```text
docs/ui/VULKAN_UI_RENDERER_CONTRACT.md
```
