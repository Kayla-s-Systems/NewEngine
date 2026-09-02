use super::*;

const SKY_CLOUD_ADVECTION_COORDS_PER_METER: f32 = 0.00075;

include!("sky_clouds/field.rs");
include!("sky_clouds/shadow_cpu.rs");
include!("sky_clouds/sampling.rs");
include!("sky_clouds/temporal.rs");
include!("sky_clouds/update.rs");
