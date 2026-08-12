#[cfg(test)]
mod in_game_editor_tests {
    use super::*;

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
