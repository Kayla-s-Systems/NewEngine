#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use blake3::Hasher;

use crate::kernel::{MathFnDesc, MathFnFlags, MathFnId, MathValue, TypeTag};

#[derive(thiserror::Error, Debug)]
pub enum MathError {
    #[error("math: function not found: {0}")]
    NotFound(String),

    #[error("math: arity mismatch for {name}: expected {expected}, got {got}")]
    ArityMismatch { name: &'static str, expected: usize, got: usize },

    #[error("math: type mismatch for {name} at arg {index}: expected {expected:?}, got {got:?}")]
    TypeMismatch {
        name: &'static str,
        index: usize,
        expected: TypeTag,
        got: TypeTag,
    },

    #[error("math: function already registered: {0}")]
    AlreadyRegistered(String),

    #[error("math: error in {name}: {msg}")]
    Exec { name: &'static str, msg: String },
}

type MathFn = Arc<dyn Fn(&[MathValue]) -> Result<MathValue, MathError> + Send + Sync + 'static>;

#[derive(Clone)]
pub struct MathRegistry {
    // Deterministic: order by id.
    by_id: BTreeMap<MathFnId, (MathFnDesc, MathFn)>,
    // Deterministic: order by name, then id.
    by_name: BTreeMap<&'static str, BTreeSet<MathFnId>>,
    revision: u64,
}

impl Default for MathRegistry {
    fn default() -> Self {
        let mut r = Self {
            by_id: BTreeMap::new(),
            by_name: BTreeMap::new(),
            revision: 0,
        };
        r.register_builtins();
        r
    }
}

impl MathRegistry {
    #[inline]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn compute_id(desc: &MathFnDesc) -> MathFnId {
        let mut h = Hasher::new();
        h.update(desc.name.as_bytes());
        h.update(b"\0");
        for t in desc.inputs {
            h.update(&[*t as u8]);
        }
        h.update(b"\0");
        h.update(&[desc.output as u8]);
        let out = h.finalize();
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&out.as_bytes()[..8]);
        MathFnId(u64::from_le_bytes(bytes))
    }

    pub fn register(
        &mut self,
        desc: MathFnDesc,
        f: impl Fn(&[MathValue]) -> Result<MathValue, MathError> + Send + Sync + 'static,
    ) -> Result<MathFnId, MathError> {
        let id = Self::compute_id(&desc);
        if self.by_id.contains_key(&id) {
            return Err(MathError::AlreadyRegistered(format!("{}", desc.name)));
        }
        self.by_name.entry(desc.name).or_default().insert(id);
        self.by_id.insert(id, (desc, Arc::new(f)));
        self.revision = self.revision.wrapping_add(1);
        Ok(id)
    }

    /// Register or replace an existing function with the same deterministic id.
    /// This is intended for plugins that want to override specific implementations.
    pub fn register_or_replace(
        &mut self,
        desc: MathFnDesc,
        f: impl Fn(&[MathValue]) -> Result<MathValue, MathError> + Send + Sync + 'static,
    ) -> MathFnId {
        let id = Self::compute_id(&desc);
        self.by_name.entry(desc.name).or_default().insert(id);
        self.by_id.insert(id, (desc, Arc::new(f)));
        self.revision = self.revision.wrapping_add(1);
        id
    }

    pub fn resolve(&self, name: &str, inputs: &[TypeTag]) -> Option<MathFnId> {
        let ids = self.by_name.get(name)?;
        // Prefer exact signature match.
        for id in ids.iter().copied() {
            if let Some((desc, _)) = self.by_id.get(&id) {
                if desc.inputs == inputs {
                    return Some(id);
                }
            }
        }
        None
    }

    pub fn desc(&self, id: MathFnId) -> Option<&MathFnDesc> {
        self.by_id.get(&id).map(|x| &x.0)
    }

    pub fn call(&self, id: MathFnId, args: &[MathValue]) -> Result<MathValue, MathError> {
        let (desc, f) = self.by_id.get(&id).ok_or_else(|| MathError::NotFound(format!("{id:?}")))?;
        if args.len() != desc.inputs.len() {
            return Err(MathError::ArityMismatch { name: desc.name, expected: desc.inputs.len(), got: args.len() });
        }
        for (i, (arg, exp)) in args.iter().zip(desc.inputs.iter()).enumerate() {
            let got = arg.type_tag();
            if got != *exp {
                return Err(MathError::TypeMismatch { name: desc.name, index: i, expected: *exp, got });
            }
        }
        f(args).map_err(|e| match e {
            MathError::Exec { .. } => e,
            other => MathError::Exec { name: desc.name, msg: other.to_string() },
        })
    }

    pub fn call_by_name(&self, name: &str, args: &[MathValue]) -> Result<MathValue, MathError> {
        let tags: Vec<TypeTag> = args.iter().map(|v| v.type_tag()).collect();
        let id = self.resolve(name, &tags).ok_or_else(|| MathError::NotFound(name.to_string()))?;
        self.call(id, args)
    }

    pub fn iter_descs(&self) -> impl Iterator<Item=(&MathFnId, &MathFnDesc)> {
        self.by_id.iter().map(|(id, (d, _))| (id, d))
    }

    fn register_builtins(&mut self) {
        use TypeTag::*;
        let det_pure = MathFnFlags::DETERMINISTIC | MathFnFlags::PURE;

        let _ = self.register(
            MathFnDesc::new("math.f32.add", &[F32, F32], F32, det_pure, "Adds two f32 values."),
            |a| Ok(MathValue::F32(a[0].clone().as_f32().unwrap() + a[1].clone().as_f32().unwrap())),
        );

        let _ = self.register(
            MathFnDesc::new("math.vec3.add", &[Vec3, Vec3], Vec3, det_pure, "Adds two Vec3 vectors."),
            |a| {
                let x = match &a[0] {
                    MathValue::Vec3(v) => *v,
                    _ => unreachable!()
                };
                let y = match &a[1] {
                    MathValue::Vec3(v) => *v,
                    _ => unreachable!()
                };
                Ok(MathValue::Vec3(x + y))
            },
        );

        let _ = self.register(
            MathFnDesc::new("math.vec3.dot", &[Vec3, Vec3], F32, det_pure, "Dot product of two Vec3 vectors."),
            |a| {
                let x = match &a[0] {
                    MathValue::Vec3(v) => *v,
                    _ => unreachable!()
                };
                let y = match &a[1] {
                    MathValue::Vec3(v) => *v,
                    _ => unreachable!()
                };
                Ok(MathValue::F32(x.dot(y)))
            },
        );

        let _ = self.register(
            MathFnDesc::new("math.vec3.cross", &[Vec3, Vec3], Vec3, det_pure, "Cross product of two Vec3 vectors."),
            |a| {
                let x = match &a[0] {
                    MathValue::Vec3(v) => *v,
                    _ => unreachable!()
                };
                let y = match &a[1] {
                    MathValue::Vec3(v) => *v,
                    _ => unreachable!()
                };
                Ok(MathValue::Vec3(x.cross(y)))
            },
        );

        let _ = self.register(
            MathFnDesc::new("math.vec3.length", &[Vec3], F32, det_pure, "Vector length of Vec3."),
            |a| {
                let x = match &a[0] {
                    MathValue::Vec3(v) => *v,
                    _ => unreachable!()
                };
                Ok(MathValue::F32(x.length()))
            },
        );
    }
}
