#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RString, RVec};
use abi_stable::StableAbi;

use crate::types::MathType;

#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Determinism {
    StrictIEEE = 1,
    FastMathAllowed = 2,
    ImplementationDefined = 3,
}

#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CallKind {
    Pure = 1,
    Stateful = 2,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq, Eq)]
pub struct MathSignature {
    pub inputs: RVec<MathType>,
    pub outputs: RVec<MathType>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq, Eq)]
pub struct MathFnDesc {
    pub id: RString,
    pub version: u32,
    pub signature: MathSignature,

    pub determinism: Determinism,
    pub call_kind: CallKind,

    pub display_name: RString,
    pub category: RVec<RString>,
    pub tags: RVec<RString>,
    pub doc: RString,
}

impl MathFnDesc {
    #[inline]
    pub fn minimal(
        id: impl Into<RString>,
        version: u32,
        signature: MathSignature,
        determinism: Determinism,
        call_kind: CallKind,
    ) -> Self {
        let id = id.into();
        Self {
            display_name: id.clone(),
            id,
            version,
            signature,
            determinism,
            call_kind,
            category: RVec::new(),
            tags: RVec::new(),
            doc: RString::new(),
        }
    }
}