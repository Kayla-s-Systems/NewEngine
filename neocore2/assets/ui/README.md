# North Star Engine UI assets

This directory contains runtime `.neui` UI dictionaries split by ownership.

Runtime `.neui` files are binary NEF8/ListFiles with `content_kind = ui_dictionary` and a deflate-compressed XMLcentral body. They are not runtime JSON layout files and not provider-owned screen definitions.

Runtime rule:

```text
.neui NEF8/ListFile
  -> engine.assets raw bytes + ListFile envelope validation
  -> engine.assets.ui compile_document_v1
  -> engine.ui mount_surface_v1 / state patches / action dispatch
  -> engine.render consumes draw packets only
```

A UI surface is authored as a generic node-navigation document, for example XMLcentral `<UiNodeNavigationDocument>`, then compiled by `engine.assets.ui` into the `UiNodeNavigationRuntime` DTO. The runtime does not own named product screens; it only publishes `UiSurfaceNode` state.

No runtime `.layout.json`, legacy `.menu.json`, embedded provider screen, alias, or compatibility fallback is allowed.

The startup window remains a bootstrap exception and must not become a second runtime UI framework.
