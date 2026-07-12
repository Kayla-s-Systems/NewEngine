use newengine_assets::{Rgba8TextureAsset, RuntimeTextureAsset};

pub(crate) fn texture_runtime_wire(packet: &RuntimeTextureAsset) -> Vec<u8> {
    let (payload, layout) = packet.concatenated_payload_and_layout();
    let header_len = newengine_assets_api::texture_wire::RUNTIME_HEADER_LEN;
    let record_len = newengine_assets_api::texture_wire::RUNTIME_MIP_RECORD_LEN;
    let mut out = Vec::with_capacity(header_len + layout.len() * record_len + payload.len());
    out.extend_from_slice(&newengine_assets_api::texture_wire::MAGIC);
    out.extend_from_slice(&newengine_assets_api::texture_wire::VERSION_RUNTIME_V2.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&packet.format.as_wire_id().to_le_bytes());
    out.extend_from_slice(&(layout.len() as u16).to_le_bytes());
    out.extend_from_slice(&packet.width.to_le_bytes());
    out.extend_from_slice(&packet.height.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for mip in layout {
        out.extend_from_slice(&(mip.level as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&mip.width.to_le_bytes());
        out.extend_from_slice(&mip.height.to_le_bytes());
        out.extend_from_slice(&(mip.offset as u32).to_le_bytes());
        out.extend_from_slice(&(mip.byte_len as u32).to_le_bytes());
    }
    out.extend_from_slice(&payload);
    out
}

pub(crate) fn texture_rgba8_wire(packet: &Rgba8TextureAsset) -> Vec<u8> {
    let mut out =
        Vec::with_capacity(newengine_assets_api::texture_wire::HEADER_LEN + packet.rgba.len());
    out.extend_from_slice(&newengine_assets_api::texture_wire::MAGIC);
    out.extend_from_slice(&newengine_assets_api::texture_wire::VERSION_RGBA8_V1.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&packet.width.to_le_bytes());
    out.extend_from_slice(&packet.height.to_le_bytes());
    out.extend_from_slice(&(packet.rgba.len() as u32).to_le_bytes());
    out.extend_from_slice(&packet.rgba);
    out
}
