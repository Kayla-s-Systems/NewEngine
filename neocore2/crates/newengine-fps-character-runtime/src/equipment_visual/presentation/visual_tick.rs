pub(crate) fn tick_equipped_weapon_visuals(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    dt: f32,
) {
    let owners = world
        .query::<newengine_engine_runtime::gameplay::PlayerController>()
        .map(|(owner, _)| owner)
        .collect::<Vec<_>>();

    for owner in owners {
        let binding = world.get::<EquippedWeaponBinding>(owner).copied();
        let existing = existing_visual(world, owner);

        let Some(binding) = binding else {
            if existing.is_some() {
                clear_equipped_weapon_visual(world, owner);
            } else {
                let _ = world.remove::<WeaponVisualAdmissionState>(owner);
            }
            continue;
        };

        if let Some((root, visual)) = existing {
            if visual.instance_id == binding.instance_id
                && visual.item == binding.item
                && world.exists(root)
            {
                let ready_matches = matches!(
                    world.get::<WeaponVisualAdmissionState>(owner),
                    Some(WeaponVisualAdmissionState::Ready {
                        item,
                        instance_id,
                        root: ready_root,
                    }) if *item == binding.item
                        && *instance_id == binding.instance_id
                        && *ready_root == root
                );
                if !ready_matches {
                    let _ = world.insert(
                        owner,
                        WeaponVisualAdmissionState::Ready {
                            item: binding.item,
                            instance_id: binding.instance_id,
                            root,
                        },
                    );
                }
                update_weapon_attachment(world, owner, root, dt);
                continue;
            }
            clear_equipped_weapon_visual(world, owner);
        }

        let tick = world.tick();
        let avatar_root = world
            .get::<PlayerModelBinding>(owner)
            .and_then(|binding| binding.visual_root)
            .filter(|root| world.exists(*root));
        let previous_state = world.get::<WeaponVisualAdmissionState>(owner).cloned();
        let mut current_key = None;

        if let Some(WeaponVisualAdmissionState::Failed {
            key,
            class,
            next_probe_tick,
            reason,
        }) = previous_state
        {
            if weapon_visual_failure_static_matches(
                key,
                binding.item,
                binding.instance_id,
                avatar_root,
            ) {
                if tick < next_probe_tick {
                    continue;
                }

                let probed_key = weapon_visual_admission_key(world, mats, owner, binding);
                current_key = Some(probed_key);
                if weapon_visual_failure_matches(
                    key,
                    binding.item,
                    binding.instance_id,
                    avatar_root,
                    probed_key.dependency_generation,
                ) && class == WeaponVisualAdmissionFailureClass::Deterministic
                {
                    let _ = world.insert(
                        owner,
                        WeaponVisualAdmissionState::Failed {
                            key,
                            class,
                            next_probe_tick: tick.saturating_add(WEAPON_VISUAL_FAILED_PROBE_TICKS),
                            reason,
                        },
                    );
                    continue;
                }
                // Transient failures are retried only at the bounded cadence above. A changed
                // dependency generation retries immediately after this probe.
            }
        }

        let key =
            current_key.unwrap_or_else(|| weapon_visual_admission_key(world, mats, owner, binding));
        let _ = world.insert(owner, WeaponVisualAdmissionState::Pending { key });

        match spawn_equipped_weapon_visual(world, prims, mats, owner, binding) {
            Ok(root) => {
                let _ = world.insert(
                    owner,
                    WeaponVisualAdmissionState::Ready {
                        item: binding.item,
                        instance_id: binding.instance_id,
                        root,
                    },
                );
                update_weapon_attachment(world, owner, root, dt);
            }
            Err(error) => {
                let class = classify_weapon_visual_admission_failure(&error);
                let retry_ticks = match class {
                    WeaponVisualAdmissionFailureClass::Deterministic => {
                        WEAPON_VISUAL_FAILED_PROBE_TICKS
                    }
                    WeaponVisualAdmissionFailureClass::Transient => {
                        WEAPON_VISUAL_TRANSIENT_RETRY_TICKS
                    }
                };
                let _ = world.insert(
                    owner,
                    WeaponVisualAdmissionState::Failed {
                        key,
                        class,
                        next_probe_tick: tick.saturating_add(retry_ticks),
                        reason: error.clone(),
                    },
                );
                newengine_ulog_api::ulog::warn!(
                    "fps-character: equipped weapon visual admission failed player={} item={:016x} instance={} tick={} avatar_ready={} dependency_generation={:016x} class={:?} retry_after_ticks={} err='{}'",
                    owner.stable_u64(),
                    binding.item.raw(),
                    binding.instance_id.0,
                    tick,
                    avatar_root.is_some(),
                    key.dependency_generation,
                    class,
                    retry_ticks,
                    error,
                );
            }
        }
    }
}

