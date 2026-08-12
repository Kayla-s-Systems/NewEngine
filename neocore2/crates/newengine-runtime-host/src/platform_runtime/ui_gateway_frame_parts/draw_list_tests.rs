use super::draw_list_animation::LoadingSpinnerAnimationSpec;
use super::draw_list_loading::apply_loading_spinner_rotation;
use super::*;

#[test]
fn spinner_runtime_uses_continuous_elapsed_time() {
    let spec = LoadingSpinnerAnimationSpec {
        rotation_rps: 1.0,
        sprite_fps: 0.0,
        sprite_frames: None,
        sprite_columns: None,
        sprite_rows: None,
        frame_width: None,
        frame_height: None,
        source: "test",
    };

    let quarter_turn = spec.runtime(250).rotation_radians;
    assert!((quarter_turn - std::f32::consts::FRAC_PI_2).abs() < 0.000_1);
}

#[test]
fn cached_loading_spinner_rotates_without_rebuilding_texture_payload() {
    let mut draw_list = UiDrawList::new();
    let mut spinner = UiImagePaintCommand::default();
    spinner.node.node_id = "loading.spinner".to_owned();
    draw_list.paint.push(UiPaintCommand::Image(spinner));

    apply_loading_spinner_rotation(&mut draw_list, 1.25);

    let UiPaintCommand::Image(spinner) = &draw_list.paint.commands[0] else {
        panic!("expected loading spinner image");
    };
    assert_eq!(spinner.rotation_radians, 1.25);
    assert!(draw_list.texture_delta.set.is_empty());
    assert!(draw_list.texture_delta.patches.is_empty());
}
