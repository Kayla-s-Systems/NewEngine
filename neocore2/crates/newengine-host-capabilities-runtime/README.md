# newengine-host-capabilities-runtime

Optional native provider for `engine.host.capabilities`.

It discovers OS and hardware state and returns an immutable
`HostPreInitSnapshot`. The runtime host installs this provider only when a
profile has not already supplied another PreInit provider route.
