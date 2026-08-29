# newengine-audio-client

Provider-neutral typed client for the stable `engine.audio` gateway.

It may serialize stable audio DTOs and invoke the gateway transport. It must not register a provider, own Scene/ECS state, know a native mixer, or depend on `newengine-audio-world-runtime` / `newengine-engine-runtime`.
