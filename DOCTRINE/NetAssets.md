# Networked Asset Delivery Doctrine

CoreEngine should treat asset delivery as a layered runtime capability.

## Asset sources

- Local filesystem layers for development and fast iteration.
- `.nepak` package layers for deterministic release content.
- Optional remote/cache layers for streamed or updated content.

## Runtime direction

The engine should address assets by logical path and route requests through `engine.assets`. The active asset provider decides where bytes come from: filesystem, package, cache, or remote source.

## Strategic value

A gateway-routed asset layer allows the engine to evolve toward streaming, patching, CDN-backed content, and live updates without rebuilding consumers for each storage backend.
