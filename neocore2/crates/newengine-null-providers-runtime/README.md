# newengine-null-providers-runtime

Optional first-party degraded provider composition.

This crate is **not part of the neocore2 host kernel**. It occupies render,
physics, UI and AI capability slots with visible null providers when a selected
composition explicitly wants degraded operation. A true empty host leaves those
slots empty instead.
