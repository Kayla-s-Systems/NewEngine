fn build_authored_look_pose_space(
    role: &'static str,
    base_clip: Option<PlayerAnimationRuntimeClip>,
    range_clip: Option<PlayerAnimationRuntimeClip>,
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
    eye_only: bool,
) -> Result<Option<AuthoredLookPoseSpace>, String> {
    match (base_clip, range_clip) {
        (None, None) => Ok(None),
        (Some(base), Some(range)) => {
            let coordinate_joint = look_coordinate_joint(skeleton, eye_only)?;
            AuthoredLookPoseSpace::build(
                role,
                &base,
                &range,
                skeleton,
                animation_runtime,
                coordinate_joint,
            )
            .map(Some)
        }
        (base, range) => Err(format!(
            "authored look pose-space role={role} requires base+range pair base={} range={}",
            base.as_ref()
                .map(|clip| clip.clip_ref.as_str())
                .unwrap_or("none"),
            range
                .as_ref()
                .map(|clip| clip.clip_ref.as_str())
                .unwrap_or("none")
        )),
    }
}
