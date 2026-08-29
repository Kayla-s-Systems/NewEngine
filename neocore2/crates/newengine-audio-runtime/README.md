# newengine-audio-runtime

First-party native provider for the `engine.audio` gateway.

The crate owns the OS audio output stream, decoding, clip cache, voice lifecycle,
bus gain state, and the first dynamic stereo spatialization path. No Rodio/CPAL
type crosses the public engine boundary: callers use DTOs from
`newengine-audio-api`, so another backend can replace this implementation through
normal gateway routing.

Current v1 scope: default output discovery, WAV/MP3/Ogg Vorbis/FLAC decode, 2D and
3D voices, listener state, Master/Music/SFX/UI/Dialogue/Ambience buses,
pause/resume/gain/speed/emitter position/stop, semantic UI feedback tones,
clip caching, diagnostics, and graceful fallback when no device is available.
