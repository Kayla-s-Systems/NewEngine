# newengine-ui-api

Stable engine-facing UI service contract for `engine.ui`.

The UI gateway is owned by the engine host. Concrete UI providers implement `ui.api` or a descriptor-declared provider service and advertise `ui.backend` metadata. Render, camera and gameplay systems publish UI-neutral telemetry/state to `engine.ui`; they do not talk to concrete UI providers.
