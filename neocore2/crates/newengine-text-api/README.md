# newengine-text-api

UI-owned text subdomain contract for `engine.ui.text`.

This crate is intentionally only a contract: font fallback, shaping, atlas allocation and localization are provider-owned. UI systems such as UI providers consume this gateway; renderers receive already shaped/atlased UI draw packets and do not own text layout policy.
