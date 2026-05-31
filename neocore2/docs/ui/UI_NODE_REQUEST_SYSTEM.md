# North Star UI Node Request System

`UiNodeTreeRequest` is the generative UI entrypoint for editor tools, schema inspectors and runtime modules.

```text
UiNodeTreeRequest
  surface_id
  source_kind
  theme_id / style_ref
  root: UiNodeRequest
    children[]
    bindings[]
    events[]
    layout
```

Submit it to:

```text
engine.ui / ui.apply_node_request_v1
```

The active UI provider converts the tree into the normal retained `UiSurfaceNode` surface. `.neui` remains useful for authored interfaces, but it is no longer the only way to create UI.

## Example

```json
{
  "version": 1,
  "request_id": "tool.asset_schema.material.v1",
  "surface_id": "engine.ui.editor.right_edit_window",
  "source": "engine.assets.inspect",
  "source_kind": "generated",
  "theme_id": "northstar.editor.dark",
  "root": {
    "id": "material_editor",
    "kind": "panel",
    "text": "Material",
    "children": [
      {
        "id": "base_color",
        "kind": "input",
        "text": "Base Color",
        "tooltip": "Schema-driven editable material parameter",
        "bindings": [{"property":"value","source":"AssetDocument","path":"sections.material.fields.base_color","mode":"two_way"}],
        "events": [{"trigger":"value_changed","action_id":"asset.patch.field","target_gateway":"engine.assets.edit","method":"asset.edit.patch_document_json_v1"}]
      }
    ]
  }
}
```

## Invariant

```text
Generated UI is still data.
Data enters through engine.ui.
Providers render and route events.
They do not own product-specific editor logic.
```
