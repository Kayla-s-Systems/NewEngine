# newengine-assets-api

A minimal, stable contract surface for interacting with an AssetManager-like service.

This crate intentionally contains only:

- small data-oriented enums/traits,
- a tiny `wait_ready` helper.

Concrete implementations live in `newengine-assets` (client) and/or runtime plugins.
