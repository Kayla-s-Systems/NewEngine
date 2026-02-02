#![forbid(unsafe_op_in_unsafe_fn)]

pub mod events;
pub mod id;
pub mod importers;
pub mod source;
pub mod store;
pub mod texture;
pub mod types;

pub use events::AssetEvent;
pub use id::AssetId;
pub use importers::Importer;
pub use source::{AssetSource, FileSystemSource};
pub use store::{AssetStore, BlobImporterDispatch, PumpBudget};
pub use texture::{TextureAsset, TextureDesc, TextureFormat, TextureKind, TextureMip, TextureSubresource};
pub use types::{
    Asset, AssetBlob, AssetDependency, AssetError, AssetKey, AssetState, ImporterPriority,
};