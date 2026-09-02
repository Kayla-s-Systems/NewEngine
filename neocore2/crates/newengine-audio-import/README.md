# newengine-audio-import

Research/import-side audio contracts for NorthStar.

This crate deliberately separates **container identity** from **codec identity**. It is not a realtime playback backend and it does not make third-party containers runtime asset formats.

Pipeline:

`source container -> probe/demux -> source codec -> decode -> canonical PCM -> NorthStar encoder -> NEF8 audio asset`

`YSNCD` is legacy compatibility only and is not a target of new imports.

Bink is treated as a research/import multimedia container. Direct Bink playback is intentionally outside the native audio runtime contract.
