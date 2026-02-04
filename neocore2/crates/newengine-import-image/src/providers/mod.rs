use abi_stable::std_types::{RResult, RString, RVec};

pub trait ImageProviderV1: Sync + Send + 'static {
    fn container(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn sniff(&self, bytes: &[u8]) -> bool;
    fn import(&self, bytes: &[u8]) -> RResult<RVec<u8>, RString>;
    fn describe_json(&self) -> &'static str;
}

pub struct ProviderEntry {
    pub provider: &'static dyn ImageProviderV1,
}

inventory::collect!(ProviderEntry);

#[inline]
pub fn iter_providers() -> impl Iterator<Item = &'static dyn ImageProviderV1> {
    inventory::iter::<ProviderEntry>
        .into_iter()
        .map(|e| e.provider)
}

pub mod dds;
pub mod png;

pub mod bmp;
pub mod gif;
pub mod jpeg;
pub mod tga;
pub mod webp;
