#![forbid(unsafe_op_in_unsafe_fn)]

#[path = "light_extraction_parts/registry.rs"]
mod registry;

pub(crate) use self::registry::LightExtractionProviderRegistry;
