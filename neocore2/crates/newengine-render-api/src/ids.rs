use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

macro_rules! define_id {
    ($name:ident, $vis_new:vis) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(NonZeroU32);

        impl $name {
            #[allow(dead_code)]
            #[inline]
            $vis_new fn new(v: u32) -> Self {
                Self(NonZeroU32::new(v).expect(concat!(stringify!($name), " must be non-zero")))
            }

            #[inline]
            pub fn get(self) -> u32 {
                self.0.get()
            }
        }
    };
}

define_id!(BufferId, pub);
define_id!(TextureId, pub);
define_id!(SamplerId, pub);
define_id!(ShaderId, pub);
define_id!(PipelineId, pub);
define_id!(BindGroupLayoutId, pub);
define_id!(BindGroupId, pub);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RenderTargetId(pub NonZeroU32);

impl RenderTargetId {
    #[inline]
    pub fn new(v: u32) -> Self {
        Self(NonZeroU32::new(v).expect("RenderTargetId must be non-zero"))
    }

    #[inline]
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BufferSlice {
    pub buffer: BufferId,
    pub offset: u64,
}

impl BufferSlice {
    #[inline]
    pub const fn new(buffer: BufferId, offset: u64) -> Self {
        Self { buffer, offset }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DrawArgs {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}

impl DrawArgs {
    #[inline]
    pub const fn new(vertex_count: u32) -> Self {
        Self {
            vertex_count,
            instance_count: 1,
            first_vertex: 0,
            first_instance: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DrawIndexedArgs {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
    pub first_instance: u32,
}

impl DrawIndexedArgs {
    #[inline]
    pub const fn new(index_count: u32) -> Self {
        Self {
            index_count,
            instance_count: 1,
            first_index: 0,
            vertex_offset: 0,
            first_instance: 0,
        }
    }
}
