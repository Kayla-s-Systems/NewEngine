#![forbid(unsafe_op_in_unsafe_fn)]

mod capsule;
mod cone;
mod cube;
mod cylinder;
mod disc;
mod grid;
mod plane;
mod sphere;
mod torus;

use crate::registry::PrimitiveParams;
use crate::{fnv1a_64, PrimitiveId, PrimitiveRegistry};

pub const ID_CUBE: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.primitive.cube.v1"));
pub const ID_PLANE: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.primitive.plane.v1"));
pub const ID_GRID: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.primitive.grid.v1"));
pub const ID_SPHERE_UV: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.primitive.sphere_uv.v1"));
pub const ID_CYLINDER: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.primitive.cylinder.v1"));
pub const ID_CONE: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.primitive.cone.v1"));
pub const ID_CAPSULE: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.primitive.capsule.v1"));
pub const ID_TORUS: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.primitive.torus.v1"));
pub const ID_DISC: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.primitive.disc.v1"));

#[inline]
pub fn register(r: &mut PrimitiveRegistry) {
    r.register(ID_CUBE, "Cube", PrimitiveParams::default(), cube::build);
    r.register(ID_PLANE, "Plane", PrimitiveParams::default(), plane::build);

    r.register(
        ID_GRID,
        "Grid",
        PrimitiveParams {
            subdivisions: 10,
            ..PrimitiveParams::default()
        },
        grid::build,
    );

    r.register(
        ID_SPHERE_UV,
        "Sphere",
        PrimitiveParams {
            slices: 32,
            stacks: 16,
            ..PrimitiveParams::default()
        },
        sphere::build,
    );

    r.register(
        ID_CYLINDER,
        "Cylinder",
        PrimitiveParams {
            segments: 32,
            ..PrimitiveParams::default()
        },
        cylinder::build,
    );

    r.register(
        ID_CONE,
        "Cone",
        PrimitiveParams {
            segments: 32,
            ..PrimitiveParams::default()
        },
        cone::build,
    );

    r.register(
        ID_CAPSULE,
        "Capsule",
        PrimitiveParams {
            segments: 32,
            rings: 8,
            ..PrimitiveParams::default()
        },
        capsule::build,
    );

    r.register(
        ID_TORUS,
        "Torus",
        PrimitiveParams {
            major_segments: 48,
            minor_segments: 16,
            ..PrimitiveParams::default()
        },
        torus::build,
    );

    r.register(
        ID_DISC,
        "Disc",
        PrimitiveParams {
            segments: 48,
            ..PrimitiveParams::default()
        },
        disc::build,
    );
}
