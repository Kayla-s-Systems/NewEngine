# NewEngine UI authored assets

This directory contains authored XMLcentral `.neui` UI dictionaries split by ownership.

Runtime rule:

```text
.neui -> engine.assets.ui compile_document_v1 -> engine.ui mount_surface_v1 -> engine.render draw packets
```

These files intentionally replace the retired runtime `.layout.json` / `.menu.json` UI assets. The startup window remains a bootstrap exception and must not become a second runtime UI framework.
