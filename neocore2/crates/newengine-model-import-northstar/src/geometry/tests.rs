    use super::*;

    #[test]
    fn northstar_triangle_winding_is_canonicalized_once() {
        let mut indices = vec![0, 1, 2, 3, 4, 5];
        reverse_northstar_triangle_winding(&mut indices);
        assert_eq!(indices, vec![0, 2, 1, 3, 5, 4]);
    }

    #[test]
    fn winding_canonicalization_flips_derived_face_normal() {
        let positions = vec![
            [0.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
        ];
        let original = recalculate_normals(&positions, &[0, 1, 2]);
        let mut indices = vec![0, 1, 2];
        reverse_northstar_triangle_winding(&mut indices);
        let canonical = recalculate_normals(&positions, &indices);
        assert!(original[0][2] > 0.99);
        assert!(canonical[0][2] < -0.99);
    }

    #[test]
    fn lsb_reader_matches_northstar_packing_order() {
        let bytes = [0b1011_0010u8, 0b0000_0011];
        let mut bits = LsbBitReader::new(&bytes);
        assert_eq!(bits.read(4).unwrap(), 0b0010);
        assert_eq!(bits.read(4).unwrap(), 0b1011);
        assert_eq!(bits.read(2).unwrap(), 0b11);
    }

    #[test]
    fn indexed_compaction_discards_only_dead_vertices() {
        let positions = vec![
            [0.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [9.0, 9.0, 9.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
        ];
        let uv0 = positions.clone();
        let (p, uv, indices, source) =
            compact_indexed_vertex_streams(&positions, &uv0, &[0, 1, 3], "test").unwrap();
        assert_eq!(source, vec![0, 1, 3]);
        assert_eq!(indices, vec![0, 1, 2]);
        assert_eq!(p.len(), 3);
        assert_eq!(uv.len(), 3);
        assert_eq!(p[2], positions[3]);
    }

    #[test]
    fn lod_parser_selects_shape_zero() {
        assert_eq!(lod_index("abby_head_lod0_LODShape0_shader0"), 0);
        assert_eq!(lod_index("abby_head_lod0_LODShape3_shader0"), 3);
    }
