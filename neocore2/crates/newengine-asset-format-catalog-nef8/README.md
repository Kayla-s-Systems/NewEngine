# newengine-asset-format-catalog-nef8

Profile/composition helper for registering first-party NEF8/ListFile format descriptor crates.

This crate is not the file type registry and does not own format semantics. Each
format crate owns its own descriptor; this catalog only links selected first-party
format crates into a runtime profile and asks them for descriptors.
