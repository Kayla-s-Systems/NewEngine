#![forbid(unsafe_op_in_unsafe_fn)]

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct ServiceDescribe {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub asset_importer: Option<AssetImporterDesc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AssetImporterDesc {
    pub extensions: Vec<String>,
    pub output_type_id: String,
    pub format: String,
    pub method: String,
    #[serde(default)]
    pub priority: Option<i32>,
    #[serde(default)]
    pub wire: Option<String>,
}

#[inline]
pub(crate) fn parse_describe(describe_json: &str) -> Option<ServiceDescribe> {
    serde_json::from_str(describe_json).ok()
}

#[inline]
pub(crate) fn is_asset_importer(describe_json: &str) -> bool {
    let Some(d) = parse_describe(describe_json) else {
        return false;
    };
    d.kind.as_deref() == Some("asset_importer") && d.asset_importer.is_some()
}
