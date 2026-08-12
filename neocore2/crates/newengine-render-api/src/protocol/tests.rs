use super::*;
use crate::{
    Extent2D, RenderDrawListKind, RenderGraphPassKind, TextureDesc, TextureId, UiDrawList,
};
use std::num::NonZeroU32;

#[test]
fn binary_multi_adapter_mesh_packet_roundtrips() {
    let mut vertices = Vec::new();
    for value in [
        1.0_f32, 2.0, 3.0, 0.0, 2.0, 0.0, 0.25, 0.75, 4.0, 5.0, 6.0, 0.0, 0.0, 4.0, 0.5, 1.0,
    ] {
        vertices.extend_from_slice(&value.to_le_bytes());
    }
    let request = MultiAdapterMeshTranscodeRequest::new(vertices.clone()).unwrap();
    let request = decode_multi_adapter_mesh_transcode_request(
        &encode_multi_adapter_mesh_transcode_request(&request).unwrap(),
    )
    .unwrap();
    assert_eq!(request.vertex_count(), 2);
    assert_eq!(request.vertex_bytes, vertices);

    let response = MultiAdapterMeshTranscodeResult {
        worker_index: 1,
        invalid_vertex_count: 2,
        gpu_elapsed_ns: 42_000,
        vertex_bytes: vertices,
    };
    let response = decode_multi_adapter_mesh_transcode_result(
        &encode_multi_adapter_mesh_transcode_result(&response).unwrap(),
    )
    .unwrap();
    assert_eq!(response.worker_index, 1);
    assert_eq!(response.invalid_vertex_count, 2);
    assert_eq!(response.gpu_elapsed_ns, 42_000);
    assert_eq!(response.vertex_count(), 2);
}

#[test]
fn binary_create_texture_roundtrips_payload_and_response() {
    let desc = TextureDesc::new(
        Extent2D::new(8, 4),
        crate::TextureFormat::Bc3RgbaSrgb,
        crate::TextureUsage::Sampled,
    )
    .with_label("binary-texture")
    .with_mips(NonZeroU32::new(2).unwrap())
    .with_deferred_mip_data(
        vec![
            crate::TextureMipDataDesc::new(0, 8, 4, 0, 32),
            crate::TextureMipDataDesc::new(1, 4, 2, 32, 16),
        ],
        (0_u8..48).collect(),
    );
    let encoded = encode_create_texture_bin(&desc).unwrap();
    let decoded = decode_create_texture_bin(&encoded).unwrap();
    assert_eq!(decoded.label.as_deref(), Some("binary-texture"));
    assert_eq!(decoded.extent.width, 8);
    assert_eq!(decoded.extent.height, 4);
    assert_eq!(decoded.format, crate::TextureFormat::Bc3RgbaSrgb);
    assert_eq!(decoded.usage, crate::TextureUsage::Sampled);
    assert_eq!(decoded.mip_levels.get(), 2);
    assert_eq!(decoded.data_policy, crate::TextureDataPolicy::Deferred);
    assert_eq!(decoded.mip_data.len(), 2);
    assert_eq!(decoded.data.as_ref().map(Vec::len), Some(48));

    let id = TextureId::new(77);
    assert_eq!(
        decode_texture_id_bin(&encode_texture_id_bin(id)).unwrap(),
        id
    );
}

#[test]
fn binary_unit_batch_roundtrips_ui_draw_list() {
    let mut ui = UiDrawList::new();
    ui.screen_size_px = [320, 200];
    ui.pixels_per_point = 1.0;

    let encoded =
        encode_unit_command_batch_bin(&[RenderCommand::SetUiDrawList(Box::new(ui))]).unwrap();
    let decoded = decode_unit_command_batch_bin(&encoded).unwrap();
    match &decoded[0] {
        RenderCommand::SetUiDrawList(list) => assert_eq!(list.screen_size_px, [320, 200]),
        other => panic!("expected SetUiDrawList, got {other:?}"),
    }
}

#[test]
fn binary_unit_batch_roundtrips_recording_scope_commands() {
    let encoded = encode_unit_command_batch_bin(&[
        RenderCommand::SetDrawListKind {
            kind: Some(RenderDrawListKind::OpaqueForward),
        },
        RenderCommand::SetRenderPhase {
            phase: Some(RenderGraphPassKind::UiComposite),
        },
        RenderCommand::SetDrawListKind { kind: None },
        RenderCommand::DiscardRecordedCommands,
    ])
    .unwrap();
    let decoded = decode_unit_command_batch_bin(&encoded).unwrap();

    assert!(matches!(
        decoded[0],
        RenderCommand::SetDrawListKind {
            kind: Some(RenderDrawListKind::OpaqueForward)
        }
    ));
    assert!(matches!(
        decoded[1],
        RenderCommand::SetRenderPhase {
            phase: Some(RenderGraphPassKind::UiComposite)
        }
    ));
    assert!(matches!(
        decoded[2],
        RenderCommand::SetDrawListKind { kind: None }
    ));
    assert!(matches!(decoded[3], RenderCommand::DiscardRecordedCommands));
}
