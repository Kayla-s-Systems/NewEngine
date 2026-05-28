#![forbid(unsafe_op_in_unsafe_fn)]

// AssetServiceClient is part of the stable engine.assets API surface.
// Keep this re-export so older engine-side imports do not duplicate the client implementation.
pub use newengine_assets_api::AssetServiceClient;
