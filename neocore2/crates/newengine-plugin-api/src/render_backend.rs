#![forbid(unsafe_op_in_unsafe_fn)]

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderBackendDescriptorV1 {
    pub abi_version: u32,

    pub id_ptr: *const u8,
    pub id_len: usize,

    pub name_ptr: *const u8,
    pub name_len: usize,

    pub version_ptr: *const u8,
    pub version_len: usize,

    pub aliases_ptr: *const u8,
    pub aliases_len: usize,

    pub default_settings_ptr: *const u8,
    pub default_settings_len: usize,
}

pub const RENDER_BACKEND_DESCRIPTOR_ABI_V1: u32 = 1;
pub const RENDER_BACKEND_DESCRIBE_SYMBOL: &[u8] = b"newengine_render_backend_describe_v1\0";
