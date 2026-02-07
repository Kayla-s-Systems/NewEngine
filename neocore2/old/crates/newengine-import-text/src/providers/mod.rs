pub struct TextMetaV1 {
    pub container: &'static str,
    pub mime: &'static str,
    pub encoding: &'static str,
    pub is_utf8: bool,
}

pub trait TextProviderV1: Sync + Send + 'static {
    fn service_id(&self) -> &'static str;
    fn container(&self) -> &'static str;

    fn extensions(&self) -> &'static [&'static str];

    fn mime(&self) -> &'static str;

    fn sniff(&self, _bytes: &[u8]) -> bool {
        true
    }

    fn meta(&self, bytes: &[u8]) -> TextMetaV1 {
        let is_utf8 = std::str::from_utf8(bytes).is_ok();
        TextMetaV1 {
            container: self.container(),
            mime: self.mime(),
            encoding: if is_utf8 { "utf-8" } else { "binary" },
            is_utf8,
        }
    }

    fn describe_json(&self) -> &'static str;
}

pub struct ProviderEntry {
    pub provider: &'static dyn TextProviderV1,
}

inventory::collect!(ProviderEntry);

#[inline]
pub fn iter_providers() -> impl Iterator<Item=&'static dyn TextProviderV1> {
    inventory::iter::<ProviderEntry>
        .into_iter()
        .map(|e| e.provider)
}

pub mod html;
pub mod json;
pub mod txt;
pub mod ui;
pub mod xml;

pub mod csv;
pub mod ini;
pub mod ron;
pub mod toml;
pub mod yaml;

pub mod glsl;
pub mod hlsl;
pub mod shader_stage;
pub mod wgsl;
