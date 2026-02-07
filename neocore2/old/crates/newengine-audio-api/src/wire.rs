use bytemuck::{bytes_of, Pod};

/// Encodes a POD value into a byte vector.
#[inline]
pub fn encode_pod<T: Pod>(v: &T) -> Vec<u8> {
    bytes_of(v).to_vec()
}

/// Decodes a POD value from an exact-sized byte slice.
#[inline]
pub fn decode_pod<T: Pod>(bytes: &[u8]) -> Result<T, &'static str> {
    if bytes.len() != core::mem::size_of::<T>() {
        return Err("bad payload size");
    }

    let mut out = core::mem::MaybeUninit::<T>::uninit();
    // Safe: T is Pod, destination is properly aligned in local stack.
    let dst = unsafe {
        core::slice::from_raw_parts_mut(out.as_mut_ptr() as *mut u8, core::mem::size_of::<T>())
    };
    dst.copy_from_slice(bytes);
    // Safe: fully initialized by copy_from_slice.
    Ok(unsafe { out.assume_init() })
}
