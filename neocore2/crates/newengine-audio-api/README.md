# newengine-audio-api

Stable engine-facing audio gateway contract for `engine.audio`.

The runtime publishes semantic audio events such as UI feedback, gameplay cues,
and ambience requests through this API. Concrete audio providers implement
`audio.api` or a vendor service with `audio.backend` metadata and are routed by
the gateway registry.
