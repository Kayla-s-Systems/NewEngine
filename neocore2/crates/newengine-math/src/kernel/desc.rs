#![forbid(unsafe_op_in_unsafe_fn)]

use crate::kernel::TypeTag;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MathFnId(pub u64);

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MathFnFlags: u32 {
        const DETERMINISTIC = 1 << 0;
        const PURE = 1 << 1;
        const EXPENSIVE = 1 << 2;
        const FAST = 1 << 3;
    }
}

#[derive(Clone, Debug)]
pub struct MathFnDesc {
    pub name: &'static str,
    pub inputs: &'static [TypeTag],
    pub output: TypeTag,
    pub flags: MathFnFlags,
    pub doc: &'static str,
}

impl MathFnDesc {
    #[inline]
    pub const fn new(
        name: &'static str,
        inputs: &'static [TypeTag],
        output: TypeTag,
        flags: MathFnFlags,
        doc: &'static str,
    ) -> Self {
        Self { name, inputs, output, flags, doc }
    }
}
