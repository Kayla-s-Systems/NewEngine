#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::{PluginBinaryAssetV1, PluginRootV1Ref};

#[derive(Clone, Debug)]
pub(crate) struct PluginIconData {
    pub(crate) media_type: String,
    pub(crate) bytes: Vec<u8>,
}

const MAX_PLUGIN_ICON_BYTES: usize = 512 * 1024;
const MAX_PLUGIN_ICON_MEDIA_TYPE_BYTES: usize = 64;
const PNG_MEDIA_TYPE: &str = "image/png";

#[inline]
pub(crate) fn extract_plugin_icon(root: PluginRootV1Ref) -> Option<PluginIconData> {
    let ui_assets = root.ui_assets_v1()?();
    let icon = ui_assets.icon_small.into_option()?;
    normalize_icon(icon)
}

fn normalize_icon(icon: PluginBinaryAssetV1) -> Option<PluginIconData> {
    let bytes = icon.bytes.as_slice();
    if bytes.is_empty() || bytes.len() > MAX_PLUGIN_ICON_BYTES {
        return None;
    }

    if icon.media_type.is_empty() || icon.media_type.len() > MAX_PLUGIN_ICON_MEDIA_TYPE_BYTES {
        return None;
    }

    let media_type = icon.media_type.to_string();
    if !media_type.eq_ignore_ascii_case(PNG_MEDIA_TYPE) {
        return None;
    }

    Some(PluginIconData {
        media_type,
        bytes: bytes.to_vec(),
    })
}
