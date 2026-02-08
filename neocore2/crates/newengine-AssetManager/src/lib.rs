#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(non_local_definitions)]
#![allow(non_camel_case_types)]

mod module;
mod plugin;

// твои существующие модули ассет-системы
pub mod events;
pub mod id;
pub mod importers;
pub mod source;
pub mod store;
pub mod texture;
pub mod types;

pub mod text_reader;
pub mod audio;
pub mod model3d;