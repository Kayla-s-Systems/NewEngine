# Editor font asset pipeline

North Star UI fonts are data assets, not provider hardcode.

Canonical editor refs:

```text
ui/fonts/editor.yft@inter_variable
ui/fonts/editor.yft@granic_slab_medium
ui/fonts/editor.yft@granic_sans_bold
ui/fonts/editor.yft@pricedown_display
```

Target path:

```text
Font source/import manifest
  -> .yft NEF8/ListFile Font Dictionary
  -> engine.assets
  -> engine.ui.text / text.font_manifest_v1
  -> provider-owned atlas/cache
  -> UiDrawList glyph quads
```

Rules:

- Do not embed `.ttf` or `.otf` binaries in source patches.
- Do not make `AureliaUI` know concrete file paths outside asset refs.
- Do not use provider-local debug glyphs as the product font.
- Provider fallback is allowed only as readable emergency output when `.yft` is missing.

## Local runtime import command

Generate the runtime editor font dictionary from local source font files:

```bat
python tools\scripts\takesome.py import-ui-fonts
```

Inputs stay local and are not committed by patches:

```text
gameAssets/ui/fonts/source/
```

Output:

```text
gameAssets/ui/fonts/editor.yft
```

The active UI text provider reports success only when the runtime dictionary contains embedded face bytes:

```text
engine.ui.text: active_text_backend=harfbuzz
```
