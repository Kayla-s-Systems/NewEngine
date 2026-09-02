#![forbid(unsafe_op_in_unsafe_fn)]

mod raw_range;
pub use raw_range::*;
mod asset_error;
pub use asset_error::*;

mod asset_service_client;
pub use asset_service_client::AssetServiceClient;

mod asset_document;
pub use asset_document::*;

mod pipeline;
pub use pipeline::*;

mod file_types;
pub use file_types::*;

mod texture_assets;
pub use texture_assets::*;

mod map_assets;
pub use map_assets::*;

mod asset_lifecycle;
pub use asset_lifecycle::*;

mod asset_streaming;
pub use asset_streaming::*;

mod source_dictionary;
pub use source_dictionary::*;

include!("lib_contracts/gateway_contracts.rs");

mod asset_ref;
pub use asset_ref::*;
pub mod list_file;
pub use list_file::*;

include!("lib_contracts/backend_filetypes.rs");
include!("lib_contracts/methods.rs");
include!("lib_contracts/runtime_contracts.rs");

#[cfg(test)]
mod file_type_tests;
