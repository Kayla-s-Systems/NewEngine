pub struct AudioMetaV1 {
    pub container: &'static str,
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub frames: u64,
    pub duration_sec: f64,
}

pub trait AudioProviderV1: Sync + Send + 'static {
    fn container(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn sniff(&self, bytes: &[u8]) -> bool;
    fn probe_meta(&self, bytes: &[u8]) -> Result<AudioMetaV1, String>;

    fn describe_json(&self) -> &'static str;
}

pub struct ProviderEntry {
    pub provider: &'static dyn AudioProviderV1,
}

inventory::collect!(ProviderEntry);

#[inline]
pub fn iter_providers() -> impl Iterator<Item=&'static dyn AudioProviderV1> {
    inventory::iter::<ProviderEntry>
        .into_iter()
        .map(|e| e.provider)
}

pub mod aac;
pub mod common;
pub mod flac;
pub mod m4a;
pub mod mp3;
pub mod ogg;
pub mod wav;
