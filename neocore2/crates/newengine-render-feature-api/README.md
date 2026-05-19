# newengine-render-feature-api

Stable in-process API for profile-owned render feature providers.

This crate intentionally contains provider traits, extraction DTOs, metadata and
feature commands only. It does not own a runtime controller and it is not a
renderer backend API. Runtime crates consume `Box`/`Arc<dyn ...>` providers;
profile crates implement providers against this API.
