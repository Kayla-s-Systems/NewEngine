# newengine-asset-format-nef8

Unified first-party asset format registry for North Star Engine.

This crate replaces the previous one-crate-per-extension descriptor split. Each
format is still data-declared, but the declaration lives in one NEF8/package
registry table: extension, content kind, semantic gateway, handler, outputs and
consumer domains. The engine collects descriptors from this crate instead of
compiling dozens of tiny `newengine-asset-format-*` crates.
