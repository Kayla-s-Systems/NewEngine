# newengine-console-runtime

Optional in-process provider for the `engine.command` gateway. It owns the
command parser, dynamic service-command projection and typed CVar registry.
The Void Engine kernel does not construct or depend on this provider.
