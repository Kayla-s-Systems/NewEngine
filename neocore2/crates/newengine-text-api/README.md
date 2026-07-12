# newengine-text-api

Provider-neutral text-domain contract for `engine.ui.text`.

The crate remains an API boundary: concrete localization storage, font shaping,
atlas generation and rendering are provider-owned. The module layout now follows
the responsibilities of a complete game text system rather than keeping every
DTO in one file.

## Architecture

| Module | Responsibility | Reference-system analogue |
|---|---|---|
| `catalog` | Locale catalogs, text blocks, hash/label lookup and source priority | `TextFile`, chunk/GXT lookup |
| `format` | Numeric/sub-string substitution, control tokens, colors and input icons | `TextFormat`, message insertion |
| `messages` | Brief/subtitle/help/loading queues, dismissal and history | `messages` |
| `paged` | Bounded multi-page text splitting | `PagedText` |
| `conversion` | Human numbers, duration/time and text conversion requests | `TextConversion` |
| `font` | Font dictionary, face metrics, style roles and fallback metadata | `FontDef`, `TextFontStore` |
| `shaping` | Unicode shaping, IME, glyph runs and caret geometry | font/shaping provider |
| `layout` | Paragraph wrapping, ellipsis and measurement | `CTextLayout`, `TextFormat` |
| `atlas` | Provider-neutral glyph-atlas planning | font texture store |
| `presentation` | Text layout style and already-shaped draw batches | `TextDraw` |
| `service` | Gateway IDs, methods, capability and runtime contract | top-level `CText` facade |

Existing public DTO names and method constants remain re-exported from `lib.rs`.
New domain methods are advertised through `TextServiceInfo`, while the required
provider lifecycle methods remain unchanged for backward-compatible degradation.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-text-api`

**Role:** Stable text-domain DTOs and service contracts. No renderer, UI provider,
filesystem backend or shaping implementation may be coupled here.

## Working rules

- Keep `lib.rs` as a facade; put domain contracts in focused modules.
- Preserve provider neutrality: implementations resolve through `engine.ui.text`.
- Renderers consume shaped/atlased draw data and do not own localization or layout policy.
- Runtime assets use `.neftd`/engine-assets references rather than raw font files.
- Keep service methods versioned and DTO defaults backward compatible.

<!-- NORTHSTAR-DIR-README:END -->
