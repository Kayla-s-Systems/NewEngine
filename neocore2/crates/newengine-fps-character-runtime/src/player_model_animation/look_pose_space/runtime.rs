impl AuthoredLookRuntimeBinding {
    fn body_space(&self, state: AuthoredLookState) -> Option<&AuthoredLookPoseSpace> {
        match state {
            AuthoredLookState::Relaxed => self.relaxed.as_ref(),
            AuthoredLookState::Crouch => self.crouch.as_ref().or(self.relaxed.as_ref()),
            AuthoredLookState::Tense => self.tense.as_ref().or(self.relaxed.as_ref()),
            AuthoredLookState::CoverLowLeft => self.cover_low_left.as_ref(),
            AuthoredLookState::CoverLowRight => self.cover_low_right.as_ref(),
            AuthoredLookState::Prone => self.prone.as_ref(),
            AuthoredLookState::Supine => self.supine.as_ref(),
            AuthoredLookState::Rope => self.rope.as_ref(),
            AuthoredLookState::Ladder => self.ladder.as_ref(),
            AuthoredLookState::SwimIdle => self.swim_idle.as_ref(),
            AuthoredLookState::Injured => self.injured.as_ref(),
            AuthoredLookState::RelaxedInjured => self.relaxed_injured.as_ref(),
        }
    }

    fn projection(
        &self,
        state: AuthoredLookState,
        yaw: f32,
        pitch: f32,
    ) -> Option<AuthoredLookProjection> {
        let body = self.body_space(state)?;
        let target = [yaw, pitch];
        let body_blend = body.solve(target);
        let after_body = [
            target[0] - body_blend.projected[0],
            target[1] - body_blend.projected[1],
        ];
        let (eye_projected, residual) = if let Some(eyes) = self.eyes.as_ref() {
            let eye_blend = eyes.solve(after_body);
            (
                eye_blend.projected,
                [
                    after_body[0] - eye_blend.projected[0],
                    after_body[1] - eye_blend.projected[1],
                ],
            )
        } else {
            ([0.0, 0.0], after_body)
        };
        Some(AuthoredLookProjection {
            body_projected: body_blend.projected,
            eye_projected,
            residual,
            turn_hysteresis_radians: body.turn_hysteresis_radians,
        })
    }

    fn apply(
        &self,
        state: AuthoredLookState,
        yaw: f32,
        pitch: f32,
        pose: &mut [JointLocalPose],
    ) -> Option<AuthoredLookProjection> {
        let body = self.body_space(state)?;
        let target = [yaw, pitch];
        let body_blend = body.solve(target);
        body.apply_blend(body_blend, pose);
        let after_body = [
            target[0] - body_blend.projected[0],
            target[1] - body_blend.projected[1],
        ];
        let (eye_projected, residual) = if let Some(eyes) = self.eyes.as_ref() {
            let eye_blend = eyes.solve(after_body);
            eyes.apply_blend(eye_blend, pose);
            (
                eye_blend.projected,
                [
                    after_body[0] - eye_blend.projected[0],
                    after_body[1] - eye_blend.projected[1],
                ],
            )
        } else {
            ([0.0, 0.0], after_body)
        };
        Some(AuthoredLookProjection {
            body_projected: body_blend.projected,
            eye_projected,
            residual,
            turn_hysteresis_radians: body.turn_hysteresis_radians,
        })
    }
}

fn look_coordinate_joint(
    skeleton: &ModelSkeletonMetadata,
    eye_only: bool,
) -> Result<usize, String> {
    if eye_only {
        skeleton
            .joints
            .iter()
            .position(|joint| {
                let name = joint.name.to_ascii_lowercase();
                name == "l_eyeball" || name == "left_eyeball" || name.contains("eye_l")
            })
            .ok_or_else(|| "authored eye look requires a left-eye skeleton joint".to_owned())
    } else {
        skeleton
            .joints
            .iter()
            .position(|joint| joint.name == skeleton.anchors.head)
            .ok_or_else(|| {
                format!(
                    "authored head look anchor '{}' is absent from skeleton",
                    skeleton.anchors.head
                )
            })
    }
}
