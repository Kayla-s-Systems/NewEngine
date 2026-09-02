# newengine-asset-format-nef8

Unified first-party asset format registry for North Star Engine.

This crate replaces the previous one-crate-per-extension descriptor split. Each
format is still data-declared, but the declaration lives in one NEF8/package
registry table: extension, content kind, semantic gateway, handler, outputs and
consumer domains. The engine collects descriptors from this crate instead of
compiling dozens of tiny `newengine-asset-format-*` crates.



## NEF8 wire envelope

The canonical wire parser and encoder live in `newengine-assets-api::list_file`.
This registry crate owns format/type declarations and consumes that shared
contract; individual format modules must not duplicate binary header offsets.

NEF8 uses a bounded, self-describing power-of-two header size class:

| `size_class` | Header bytes | Defined fields |
|---:|---:|---|
| 4 | 16 | prologue, type ID, flags, schema version, entry count |
| 5 | 32 | stored body length and decompressed body length |
| 6 | 64 | full BLAKE3 hash of the decompressed body |
| 7 | 128 | stable file/import identities and reserved extension space |
| 8 | 256 | forward-compatible reserved extension space |

Readers accept only wire version `2` and size classes `4..=8`. Integers are
parsed explicitly as little-endian. Unknown extension bytes are skipped up to
the declared header boundary, while unsupported wire versions are rejected.

`type_id` is a stable enum registry, not a bit mask. Values do not need to be
powers of two. In V2 it is the little-endian `u16` at offset `0x06`; offset
`0x08` is the flags field. YBN owns ID `8`; YFD owns the distinct ID `22`.

Small resources without header metadata may use the 16-byte class. Assets with
decompressed-length information use 32 bytes; assets retaining a full BLAKE3
integrity field use 64 bytes. A 128-byte header is selected only when identity
or extension fields are actually requested.

## Internal architecture

The crate keeps one public registry while separating responsibilities:

```text
src/
├── lib.rs                 # compatibility facade and public re-exports
├── descriptor.rs          # descriptor construction and write-back policy
├── formats.rs             # data-only identities for all NEF8 extensions
├── registry.rs            # canonical static spec table and lookup
├── tests.rs               # registry contract tests
├── ydd_binary.rs          # binary YDD facade
└── ydd_binary/
    ├── types.rs           # public document model and selectors
    ├── decode.rs          # strict bounded decoder
    └── tests.rs           # binary layout regression tests
```

Public paths such as `newengine_asset_format_nef8::ydd::EXTENSION`,
`specs()`, `descriptor_for_extension()` and `ydd_binary::*` remain stable.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-asset-format-nef8`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
