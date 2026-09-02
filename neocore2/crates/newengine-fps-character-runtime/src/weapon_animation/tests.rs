#[cfg(test)]
mod tests {
    use super::*;

    fn test_reload_clip(events: Vec<newengine_animation_runtime::AnimationEvent>) -> AnimationClip {
        AnimationClip {
            name: "reload".to_owned(),
            skeleton_ref: String::new(),
            source: "test".to_owned(),
            duration_seconds: 1.2,
            sample_rate_hz: 30.0,
            looped: false,
            joint_tags: vec![0],
            events,
            poses: vec![JointLocalPose {
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: Some([1.0, 1.0, 1.0]),
            }],
        }
    }

    fn complete_reload_markers() -> Vec<newengine_animation_runtime::AnimationEvent> {
        [
            (0.20, WeaponReloadPhase::MagazineDetached),
            (0.45, WeaponReloadPhase::AmmoCommitted),
            (0.70, WeaponReloadPhase::MagazineInserted),
            (0.85, WeaponReloadPhase::Chambered),
            (1.00, WeaponReloadPhase::Complete),
        ]
        .into_iter()
        .map(|(time, phase)| {
            newengine_animation_runtime::AnimationEvent::new(
                time,
                phase.animation_marker_tag().expect("reload marker tag"),
            )
        })
        .collect()
    }

    #[test]
    fn complete_reload_marker_set_admits_animation_authority() {
        let clip = test_reload_clip(complete_reload_markers());
        let instance = ItemInstanceId(81);
        let authority = authored_reload_marker_authority(
            instance,
            WeaponReloadTopology::DetachableMagazine,
            "reload@test",
            &clip,
        )
        .expect("marker validation")
        .expect("animation authority");
        assert_eq!(authority.weapon_instance_id, instance);
        assert_eq!(authority.clip_duration_seconds, 1.2);
        assert_eq!(
            authority.marker_mask,
            WeaponReloadTopology::DetachableMagazine.required_animation_marker_mask()
        );
        assert_eq!(
            authority.required_marker_mask,
            WeaponReloadTopology::DetachableMagazine.required_animation_marker_mask()
        );
        assert!(authority.is_complete());
    }

    #[test]
    fn markerless_and_partial_reload_clips_use_timeline_fallback() {
        let markerless = test_reload_clip(Vec::new());
        assert!(authored_reload_marker_authority(
            ItemInstanceId(82),
            WeaponReloadTopology::DetachableMagazine,
            "markerless",
            &markerless,
        )
        .expect("markerless validation")
        .is_none());

        let partial = test_reload_clip(vec![newengine_animation_runtime::AnimationEvent::new(
            0.2,
            WeaponReloadPhase::MagazineDetached
                .animation_marker_tag()
                .unwrap(),
        )]);
        assert!(authored_reload_marker_authority(
            ItemInstanceId(83),
            WeaponReloadTopology::DetachableMagazine,
            "partial",
            &partial,
        )
        .expect("partial validation")
        .is_none());
    }

    #[test]
    fn internal_magazine_requires_only_semantically_relevant_reload_markers() {
        let clip = test_reload_clip(vec![
            newengine_animation_runtime::AnimationEvent::new(
                0.45,
                WeaponReloadPhase::AmmoCommitted
                    .animation_marker_tag()
                    .unwrap(),
            ),
            newengine_animation_runtime::AnimationEvent::new(
                0.85,
                WeaponReloadPhase::Chambered.animation_marker_tag().unwrap(),
            ),
            newengine_animation_runtime::AnimationEvent::new(
                1.00,
                WeaponReloadPhase::Complete.animation_marker_tag().unwrap(),
            ),
        ]);
        let authority = authored_reload_marker_authority(
            ItemInstanceId(87),
            WeaponReloadTopology::InternalMagazine,
            "internal",
            &clip,
        )
        .expect("internal marker validation")
        .expect("internal marker authority");
        assert_eq!(
            authority.required_marker_mask,
            WeaponReloadTopology::InternalMagazine.required_animation_marker_mask()
        );
        assert!(authority.is_complete());
    }

    #[test]
    fn duplicate_and_out_of_order_authoritative_markers_are_rejected() {
        let mut duplicate_events = complete_reload_markers();
        duplicate_events.insert(
            1,
            newengine_animation_runtime::AnimationEvent::new(
                0.21,
                WeaponReloadPhase::MagazineDetached
                    .animation_marker_tag()
                    .unwrap(),
            ),
        );
        let duplicate = test_reload_clip(duplicate_events);
        assert!(authored_reload_marker_authority(
            ItemInstanceId(84),
            WeaponReloadTopology::DetachableMagazine,
            "duplicate",
            &duplicate,
        )
        .expect_err("duplicate marker must fail")
        .contains("duplicate authoritative marker"));

        let out_of_order = test_reload_clip(vec![
            newengine_animation_runtime::AnimationEvent::new(
                0.20,
                WeaponReloadPhase::AmmoCommitted
                    .animation_marker_tag()
                    .unwrap(),
            ),
            newengine_animation_runtime::AnimationEvent::new(
                0.30,
                WeaponReloadPhase::MagazineDetached
                    .animation_marker_tag()
                    .unwrap(),
            ),
            newengine_animation_runtime::AnimationEvent::new(
                0.70,
                WeaponReloadPhase::MagazineInserted
                    .animation_marker_tag()
                    .unwrap(),
            ),
            newengine_animation_runtime::AnimationEvent::new(
                0.85,
                WeaponReloadPhase::Chambered.animation_marker_tag().unwrap(),
            ),
            newengine_animation_runtime::AnimationEvent::new(
                1.00,
                WeaponReloadPhase::Complete.animation_marker_tag().unwrap(),
            ),
        ]);
        assert!(authored_reload_marker_authority(
            ItemInstanceId(85),
            WeaponReloadTopology::DetachableMagazine,
            "out-of-order",
            &out_of_order,
        )
        .expect_err("out-of-order markers must fail")
        .contains("out of order"));
    }

    #[test]
    fn reload_timeline_bridge_targets_weapon_owner_inbox() {
        let mut world = newengine_ecs::World::new();
        let owner = world.spawn();
        let root = world.spawn();
        let instance = ItemInstanceId(86);
        let event = newengine_animation_api::AnimationTimelineEventV1 {
            entity: root.into(),
            clip: newengine_animation_api::AnimationClipRef("reload@test".to_owned()),
            channel: "weapon.reload".to_owned(),
            tag: newengine_tags_api::TagId::new(
                WeaponReloadPhase::AmmoCommitted
                    .animation_marker_tag()
                    .unwrap(),
            ),
            clip_time_seconds: 0.45,
            playback_time_seconds: 0.45,
            loop_index: 0,
            parameters: serde_json::Value::Null,
        };

        bridge_reload_timeline_markers(&mut world, owner, instance, &[event]);
        let markers = newengine_engine_runtime::gameplay::drain_weapon_reload_animation_markers(
            &mut world, owner, instance,
        );
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].phase, WeaponReloadPhase::AmmoCommitted);
        assert_eq!(markers[0].weapon_instance_id, instance);
    }

    #[test]
    fn shared_mount_prefix_is_not_part_of_skeleton_identity() {
        assert!(skeleton_refs_compatible(
            "models/weapon/rifle/rifle.ymt@rifle",
            "shared/models/weapon/rifle/rifle.ymt@rifle"
        ));
    }
}
