# Stabilization pass — YTYP definitionType and binary/deflate-ready source envelope

## Problem

Classifying `.ytyp` as `plainText` was too narrow. It made the temporary XML
encoding look like the format contract. That would force another architecture
change when `.ytyp` becomes native binary or compressed.

## Fix

`.ytyp` is now registered as:

```text
codec_type = definitionType
format     = newengine.ytyp.definition_entries
output     = model.definition_entries_json
```

The stable contract is **Definition Entries**, not XML text.

## Supported source encodings

The codec accepts current legacy XML:

```text
source_format   = rockstar.map_types.ytyp.xml
source_encoding = utf8.xml
```

It also reserves and implements a native binary source envelope:

```text
u8[8] magic = NEYTYP01
u16   version = 1
u16   flags         # bit 0 = deflate payload
u16   payload_kind  # 1 = XML, 2 = canonical DefinitionEntriesManifest JSON
u16   header_len    # currently 32
u64   payload_len
u64   raw_len
payload bytes
```

When `flags & 1 != 0`, payload bytes are inflated. The codec tries zlib-wrapped
deflate first and raw deflate second. `raw_len` is checked after inflate when it
is non-zero.

## Boundary rule

AssetManager still does not know XML, YTYP, binary records, or deflate. It only
loads the codec descriptor and routes `.ytyp` bytes to `asset.codec.ytyp`.

Runtime/model consumers keep calling:

```text
asset.decode_v1(output_kind=model.definition_entries_json)
engine.model / model.definition_entries_json_v1
```

No consumer changes are required when the physical `.ytyp` source changes from
XML to binary/deflate.
