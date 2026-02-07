use ash::Entry;
use std::ffi::CStr;

pub(super) unsafe fn has_instance_layer(entry: &Entry, name: &CStr) -> bool {
    let layers = entry
        .enumerate_instance_layer_properties()
        .unwrap_or_default();

    layers.iter().any(|l| {
        CStr::from_ptr(l.layer_name.as_ptr()) == name
    })
}