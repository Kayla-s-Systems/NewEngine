# newengine-asset-format-ypdb

Self-contained file type descriptor crate for `.ypdb`.

This crate declares what the format is, which extension/content kind it owns,
which codec handles it, which semantic gateway interprets it, and which runtime
domains may consume it. The central registry does not hard-code this knowledge.
