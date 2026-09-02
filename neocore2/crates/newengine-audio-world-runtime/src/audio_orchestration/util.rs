fn sanitize_weight(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn sanitize_transition(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 60.0)
    } else {
        0.0
    }
}
