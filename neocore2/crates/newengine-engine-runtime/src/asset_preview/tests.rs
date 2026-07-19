use super::geometry::{normalize_preview_geometry, texture_dimensions};
use super::*;

#[test]
fn bundle_cache_is_bounded_and_promotes_recent_hit() {
    let viewport = Arc::new(ViewportBridge::new());
    let api = AssetPreviewApi::new(viewport);
    for index in 0..(PREVIEW_BUNDLE_CACHE_CAPACITY + 2) {
        api.set_render_bundle(
            &format!("models/{index}.ydd"),
            ModelAssetBundle {
                source: format!("models/{index}.ydd"),
                properties_ref: None,
                parts: Vec::new(),
                skeleton: None,
                texture_dictionary: None,
                collisions: Vec::new(),
                configuration: ModelRuntimeConfiguration::default(),
                dependency_graph: ResolvedAssetGraphV2::default(),
            },
        );
    }
    assert_eq!(api.bundle_cache.lock().len(), PREVIEW_BUNDLE_CACHE_CAPACITY);
    assert!(!api.activate_cached_bundle("models/0.ydd"));
    assert!(api.activate_cached_bundle("models/4.ydd"));
    assert_eq!(
        api.bundle_cache.lock().front().unwrap().asset_ref,
        "models/4.ydd"
    );
}

#[test]
fn texture_dimensions_use_manifest_metadata_without_rgba_decode() {
    let document = AssetDocument::default();
    let metadata = std::collections::BTreeMap::from([
        ("width".to_owned(), "4096".to_owned()),
        ("height".to_owned(), "2048".to_owned()),
    ]);
    assert_eq!(texture_dimensions(&document, Some(&metadata)), (4096, 2048));
}

#[test]
fn preview_camera_orbit_and_zoom_are_clamped() {
    let camera = AssetPreviewCameraState::default();
    let initial = camera.snapshot();
    let orbited = camera.orbit(80.0, -50.0);
    assert_ne!(initial.yaw_radians, orbited.yaw_radians);
    assert!(orbited.pitch_radians >= PREVIEW_MIN_PITCH);
    assert!(orbited.pitch_radians <= PREVIEW_MAX_PITCH);
    assert!(camera.zoom(1200.0).distance >= PREVIEW_MIN_DISTANCE);
    assert!(camera.zoom(-12000.0).distance <= PREVIEW_MAX_DISTANCE);
}

#[test]
fn preview_camera_middle_pan_moves_target_in_camera_plane() {
    let camera = AssetPreviewCameraState::default();
    let initial = camera.snapshot();
    let panned = camera.pan(80.0, -40.0);
    assert_ne!(initial.target_offset, panned.target_offset);
    assert!(panned.target_offset.iter().all(|value| value.is_finite()));
}

#[test]
fn preview_camera_reset_restores_default_view() {
    let camera = AssetPreviewCameraState::default();
    camera.orbit(40.0, 30.0);
    camera.pan(60.0, -25.0);
    camera.zoom(2.0);
    assert_eq!(camera.reset(), AssetPreviewView::default());
}

#[test]
fn preview_geometry_normalization_uses_vertex_aabb_not_coarse_sphere_bounds() {
    let mut parts = vec![ModelMeshPart {
        material_slot: "default".to_owned(),
        mesh: newengine_primitives::PrimitiveMesh {
            vertices: vec![
                newengine_primitives::PrimitiveVertex {
                    pos: [100.0, 200.0, -50.0],
                    ..Default::default()
                },
                newengine_primitives::PrimitiveVertex {
                    pos: [700.0, 220.0, -40.0],
                    ..Default::default()
                },
            ],
            indices: vec![0, 1],
            bounds_center: Vec3::new(84.0, 136.0, -75.0),
            bounds_radius: 323.0,
        },
        material: ModelMaterialBinding::default(),
    }];

    let summary = normalize_preview_geometry(&mut parts).unwrap();
    assert!((summary.source_extent.x - 600.0).abs() < 0.001);
    let positions = parts[0]
        .mesh
        .vertices
        .iter()
        .map(|vertex| Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]))
        .collect::<Vec<_>>();
    let normalized_min = positions
        .iter()
        .fold(Vec3::splat(f32::INFINITY), |a, b| a.min(*b));
    let normalized_max = positions
        .iter()
        .fold(Vec3::splat(f32::NEG_INFINITY), |a, b| a.max(*b));
    assert!(((normalized_max - normalized_min).x - 2.2).abs() < 0.001);
    assert!(((normalized_min + normalized_max) * 0.5).length() < 0.001);
    assert!(parts[0].mesh.bounds_radius < 2.0);
}

#[test]
fn ytd_root_is_classified_as_texture_preview_without_descriptor() {
    let viewport = Arc::new(ViewportBridge::new());
    let api = AssetPreviewApi::new(viewport);
    let document = AssetDocument {
        asset_ref: "textures/characters/sarah_color.ytd".to_owned(),
        ..AssetDocument::default()
    };
    assert!(api.is_texture(&document));
}

#[test]
fn ydd_and_ydr_are_classified_as_model_preview_without_descriptor() {
    let viewport = Arc::new(ViewportBridge::new());
    let api = AssetPreviewApi::new(viewport);
    for asset_ref in ["models/props/tools/axe.ydd", "models/props/tool.ydr"] {
        let document = AssetDocument {
            asset_ref: asset_ref.to_owned(),
            ..AssetDocument::default()
        };
        assert!(api.is_model(&document), "{asset_ref}");
    }
}

#[test]
fn nemat_root_is_classified_as_material_preview() {
    let viewport = Arc::new(ViewportBridge::new());
    let api = AssetPreviewApi::new(viewport);
    let document = AssetDocument {
        asset_ref: "materials/tool_axe.nemat".to_owned(),
        content_kind: Some(LIST_FILE_CONTENT_KIND_NEMAT),
        asset_kind: "list_file".to_owned(),
        semantic_gateway: "engine.assets".to_owned(),
        ..AssetDocument::default()
    };

    assert!(api.is_material(&document));
}

#[test]
fn nemat_selector_is_classified_as_material_preview_without_descriptor() {
    let viewport = Arc::new(ViewportBridge::new());
    let api = AssetPreviewApi::new(viewport);
    let document = AssetDocument {
        asset_ref: "materials/tool_axe.nemat@axe_head".to_owned(),
        ..AssetDocument::default()
    };

    assert!(api.is_material(&document));
}

#[test]
fn clearing_preview_releases_external_extent_and_texture_id() {
    let viewport = Arc::new(ViewportBridge::new());
    let api = AssetPreviewApi::new(Arc::clone(&viewport));
    viewport.publish_external_extent(252, 118);
    viewport.publish_tex_user(77);

    api.clear();

    assert!(!viewport.external_extent_owned());
    assert_eq!(viewport.read_extent(), (0, 0));
    assert_eq!(viewport.read_tex_user(), 0);
}
