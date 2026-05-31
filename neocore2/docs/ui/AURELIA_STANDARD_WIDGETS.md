# Aurelia Standard Widgets

Aurelia renders provider-neutral UI atoms from `UiComponentNode` / `UiNodeRequest`.

Supported visual atoms:

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

The provider must not know products such as Asset Browser or EditorScreen. Product modules publish UI nodes and interaction state; Aurelia paints the resulting generic atoms.
