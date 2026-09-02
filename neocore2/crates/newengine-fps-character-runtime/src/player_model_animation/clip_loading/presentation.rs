fn load_equipment_pose_sets(
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
) -> Result<(
    EquipmentPoseSet,
    std::collections::BTreeMap<String, EquipmentPoseSet>,
), String> {
    // Optional means not authored only. Once a character definition publishes a reference,
    // that clip is part of the presentation contract and must decode/bind successfully.
    // Silent locomotion/bind substitution would create an animation gap and visible reset.
    let equipment_default_pose_set = EquipmentPoseSet {
        ready: load_authored_presentation_clip(
            "equipment_ready",
            assignment.animation_for_slot("equipment.ready"),
            assignment,
            skeleton,
            &animation_runtime,
        )?,
        aim: load_authored_presentation_clip(
            "equipment_aim",
            assignment.animation_for_slot("equipment.aim"),
            assignment,
            skeleton,
            &animation_runtime,
        )?,
        reload: load_authored_presentation_clip(
            "equipment_reload",
            assignment.animation_for_slot("equipment.reload"),
            assignment,
            skeleton,
            &animation_runtime,
        )?,
        ready_sample_phase: None,
        ..EquipmentPoseSet::default()
    };
    // Family ids remain authored opaque data. Any recognized `equipment.<family>.*` capability
    // admits that family; runtime interprets only the universal stance/direction/layer grammar.
    let mut equipment_pose_families = std::collections::BTreeSet::new();
    for slot in assignment.animation_slots.keys() {
        let normalized = slot.trim().to_ascii_lowercase();
        let mut segments = normalized.split('.');
        if segments.next() != Some("equipment") {
            continue;
        }
        let Some(family) = segments.next().filter(|family| !family.is_empty()) else {
            continue;
        };
        if segments.next().is_some() {
            equipment_pose_families.insert(family.to_owned());
        }
    }
    let mut equipment_pose_sets = std::collections::BTreeMap::new();
    for family in equipment_pose_families {
        let load_family_clip =
            |semantic: &str| -> Result<Option<PlayerAnimationRuntimeClip>, String> {
                let slot = format!("equipment.{family}.{semantic}");
                let role = format!("equipment_{}_{}", family, semantic.replace('.', "_"));
                load_authored_presentation_clip(
                    &role,
                    assignment.animation_for_slot(&slot),
                    assignment,
                    skeleton,
                    &animation_runtime,
                )
            };

        // A classified weapon never inherits another class's generic pose. Missing authored
        // layers remain absent and fail closed; compatibility fields are family-local only.
        let mut set = EquipmentPoseSet::default();
        set.ready_sample_phase = assignment
            .presentation
            .equipment_ready_sample_phases
            .get(&family)
            .copied()
            .filter(|phase| phase.is_finite())
            .map(|phase| phase.clamp(0.0, 1.0));
        set.ready = load_family_clip("ready")?;
        set.aim = load_family_clip("aim")?;
        set.reload = load_family_clip("reload")?;
        set.transitions.ready_to_aim = load_family_clip("transition.ready_to_aim")?;
        set.transitions.aim_to_ready = load_family_clip("transition.aim_to_ready")?;

        for stance in [
            EquipmentPoseBodyStance::Stand,
            EquipmentPoseBodyStance::Crouch,
            EquipmentPoseBodyStance::Prone,
        ] {
            let stance_name = match stance {
                EquipmentPoseBodyStance::Stand => "stand",
                EquipmentPoseBodyStance::Crouch => "crouch",
                EquipmentPoseBodyStance::Prone => "prone",
            };
            let grip_prefix = format!("grip.{stance_name}");
            let aim_prefix = format!("{}aim", stance.semantic_prefix());
            let pose_space = set.pose_space_mut(stance);
            pose_space.grip.reference = load_family_clip(&format!("{grip_prefix}.ref"))?;
            pose_space.grip.arms = load_family_clip(&format!("{grip_prefix}.arms"))?;
            pose_space.grip.hands = load_family_clip(&format!("{grip_prefix}.hands"))?;
            pose_space.grip.fingers = load_family_clip(&format!("{grip_prefix}.fingers"))?;
            pose_space.grip.additive = load_family_clip(&format!("{grip_prefix}.add"))?;
            pose_space.idle = load_family_clip(&format!("{aim_prefix}.idle"))?;
            pose_space.blocked_additive = load_family_clip(&format!("{aim_prefix}.blocked.add"))?;
            pose_space.blocked_subtractive =
                load_family_clip(&format!("{aim_prefix}.blocked.sub"))?;
            for direction in EquipmentAimDirection::ALL {
                if let Some(clip) =
                    load_family_clip(&format!("{aim_prefix}.move.{}", direction.semantic()))?
                {
                    pose_space.movement.insert(direction, clip);
                }
            }
        }
        if set.any() {
            equipment_pose_sets.insert(family, set);
        }
    }
    Ok((equipment_default_pose_set, equipment_pose_sets))
}

fn load_authored_look_binding(
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
) -> Result<AuthoredLookRuntimeBinding, String> {
    let look_slot = |semantic: &str| assignment.animation_slots.get(semantic).map(String::as_str);
    let load_look_clip = |role: &str, semantic: &str| {
        load_authored_presentation_clip(
            role,
            look_slot(semantic),
            assignment,
            skeleton,
            &animation_runtime,
        )
    };
    let build_look_space =
        |role: &'static str, base_semantic: &str, range_semantic: &str, eye_only: bool| {
            build_authored_look_pose_space(
                role,
                load_look_clip(base_semantic, base_semantic)?,
                load_look_clip(range_semantic, range_semantic)?,
                skeleton,
                &animation_runtime,
                eye_only,
            )
        };
    let authored_look = AuthoredLookRuntimeBinding {
        relaxed: build_look_space("relaxed", "look.relaxed.base", "look.relaxed.range", false)?,
        crouch: build_look_space("crouch", "look.crouch.base", "look.crouch.range", false)?,
        tense: build_look_space("tense", "look.tense.base", "look.tense.range", false)?,
        cover_low_left: build_look_space(
            "cover_low_left",
            "look.context.cover_low_left.base",
            "look.context.cover_low_left.range",
            false,
        )?,
        cover_low_right: build_look_space(
            "cover_low_right",
            "look.context.cover_low_right.base",
            "look.context.cover_low_right.range",
            false,
        )?,
        prone: build_look_space(
            "prone",
            "look.context.prone.base",
            "look.context.prone.range",
            false,
        )?,
        supine: build_look_space(
            "supine",
            "look.context.supine.base",
            "look.context.supine.range",
            false,
        )?,
        rope: build_look_space(
            "rope",
            "look.context.rope.base",
            "look.context.rope.range",
            false,
        )?,
        ladder: build_look_space(
            "ladder",
            "look.context.ladder.base",
            "look.context.ladder.range",
            false,
        )?,
        swim_idle: build_look_space(
            "swim_idle",
            "look.context.swim_idle.base",
            "look.context.swim_idle.range",
            false,
        )?,
        injured: build_look_space(
            "injured",
            "look.context.injured.base",
            "look.context.injured.range",
            false,
        )?,
        relaxed_injured: build_look_space(
            "relaxed_injured",
            "look.context.relaxed_injured.base",
            "look.context.relaxed_injured.range",
            false,
        )?,
        eyes: build_look_space("eyes", "look.eyes.base", "look.eyes.range", true)?,
    };
    Ok(authored_look)
}

