#![forbid(unsafe_op_in_unsafe_fn)]

pub mod builtins;

mod component;
mod id;
mod mesh;
mod registry;
mod vertex;

pub use component::Primitive;
pub use id::{fnv1a_64, PrimitiveId};
pub use mesh::PrimitiveMesh;
pub use registry::{PrimitiveBuildError, PrimitiveBuildFn, PrimitiveDesc, PrimitiveParams, PrimitiveRegistry};
pub use vertex::PrimitiveVertex;

#[cfg(test)]
mod tests {
    use super::*;

    fn validate_mesh(m: &PrimitiveMesh) {
        assert!(!m.vertices.is_empty());
        assert!(!m.indices.is_empty());
        assert!(m.indices.len() % 3 == 0);
        let vlen = m.vertices.len() as u32;
        for &ix in &m.indices {
            assert!(ix < vlen);
        }
    }

    #[test]
    fn builtins_build() {
        let r = PrimitiveRegistry::with_builtins();
        for id in r.ids() {
            let m = r.build_mesh(id).unwrap();
            validate_mesh(&m);
        }
    }

    #[test]
    fn sphere_param_override() {
        let r = PrimitiveRegistry::with_builtins();
        let m = r
            .build_mesh_with(
                builtins::ID_SPHERE_UV,
                &PrimitiveParams {
                    slices: 8,
                    stacks: 4,
                    ..PrimitiveParams::default()
                },
            )
            .unwrap();
        validate_mesh(&m);
    }
}
