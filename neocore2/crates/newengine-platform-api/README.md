# newengine-platform-api

Stable DTO and service constants for the `engine.platform` gateway.

The current host-owned baseline exposes native window handles and surface metrics through `window_snapshot_json_v1`. Runtime/provider consumers should call `engine.platform`; window data is a method payload, not a separate service id.
