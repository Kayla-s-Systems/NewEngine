# newengine-asset-format-common

Shared descriptor helpers for self-declared NEF8/ListFile asset format crates.

This crate does not own any format identity. Each format crate still declares its
own extension, content kind, asset kind, semantic gateway and selector syntax.
The common crate only removes repeated descriptor boilerplate.
