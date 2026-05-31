# North Star Editor Fonts

This directory declares the editor font family as a North Star ListFile asset.

Runtime reference:

```text
ui/fonts/editor.yft@inter_variable
ui/fonts/editor.yft@granic_slab_medium
ui/fonts/editor.yft@granic_sans_bold
ui/fonts/editor.yft@pricedown_display
```

`*.yft` is a NEF8/ListFile font dictionary. It describes font faces,
roles, source provenance and atlas policy. Font binaries are source/import
inputs, not engine code and not UI-provider hardcode.

Expected local importer inputs live under:

```text
assets/ui/fonts/source/
```

Do not make the UI provider depend on raw `.ttf`/`.otf` files directly.
The correct path is:

```text
source font file -> font importer -> ui/fonts/editor.yft -> engine.ui.text -> engine.ui
```
