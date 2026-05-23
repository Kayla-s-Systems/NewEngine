# NewEngine UI assets

This directory contains runtime `.neui` UI dictionaries split by ownership.

Runtime `.neui` files are binary NEF8/ListFiles with `content_kind = ui_dictionary` and a deflate-compressed XMLcentral body. They are not runtime JSON layout/menu files.

Runtime rule:

```text
.neui NEF8/ListFile
  -> engine.assets raw bytes + ListFile envelope validation
  -> engine.assets.ui compile_document_v1
  -> engine.ui mount_surface_v1 / state patches / action dispatch
  -> engine.render consumes draw packets only
```

The pause menu navigation document is authored inside `assets/ui/engine/pause_menu.neui` as XMLcentral `<MenuDocument>`, then compiled by `engine.assets.ui` into the legacy-free runtime `MenuRuntime` DTO. No runtime `.layout.json` / `.menu.json` fallback is allowed.

The startup window remains a bootstrap exception and must not become a second runtime UI framework.
