#[cfg(test)]
mod in_game_editor_tests {
    use super::*;

    #[test]
    fn runtime_render_child_selection_promotes_to_transform_edit_root() {
        let bridge = SceneBridge::new(newengine_scene::Scene::new());
        let (root, child) = {
            let scene_lock = bridge.scene();
            let mut scene = scene_lock.write();
            let world = scene.world_mut();
            let root = world.spawn();
            let child = world.spawn();
            let _ = world.insert(root, Transform::default());
            let _ = world.insert(root, newengine_transform_api::TransformEditRoot);
            let _ = world.insert(child, Transform::default());
            assert!(newengine_transform::set_parent(world, child, Some(root)));
            (root, child)
        };

        bridge.set_selection(Some(child));
        assert_eq!(bridge.selection(), Some(root));
        assert_eq!(bridge.selections(), vec![root]);
    }

    #[test]
    fn parses_transform_action_fields() {
        assert_eq!(
            TransformEditField::parse("game.editor.transform.position.x"),
            Some(TransformEditField::PositionX)
        );
        assert_eq!(
            TransformEditField::parse("game.editor.transform.rotation.y"),
            Some(TransformEditField::RotationY)
        );
        assert!(TransformEditField::parse("game.editor.transform.unknown").is_none());
    }

    #[test]
    fn parses_numeric_and_text_action_values() {
        assert_eq!(
            action_payload_f32(&serde_json::json!({"value": 12.5})),
            Some(12.5)
        );
        assert_eq!(
            action_payload_f32(&serde_json::json!({"value": "-3.25"})),
            Some(-3.25)
        );
        assert!(action_payload_f32(&serde_json::json!({"value": "not-a-number"})).is_none());
    }

    #[test]
    fn transform_rotation_uses_xyz_degrees_contract() {
        let mut transform = Transform::default();
        TransformEditField::RotationY.apply(&mut transform, 90.0);
        let (yaw, pitch, roll) = transform.yaw_pitch_roll();
        assert!((yaw.to_degrees() - 90.0).abs() < 0.001);
        assert!(pitch.abs() < 0.001);
        assert!(roll.abs() < 0.001);
    }
}
