#[cfg(test)]
mod primitive_pass_slice_tests {
    use super::PrimitivePassSlice;

    #[test]
    fn forward_partition_keeps_decals_out_of_world_slice() {
        assert!(PrimitivePassSlice::NonDecal.accepts(false));
        assert!(!PrimitivePassSlice::NonDecal.accepts(true));
        assert!(PrimitivePassSlice::DecalOnly.accepts(true));
        assert!(!PrimitivePassSlice::DecalOnly.accepts(false));
        assert!(PrimitivePassSlice::All.accepts(false));
        assert!(PrimitivePassSlice::All.accepts(true));
    }
}
