    use super::*;

    fn room() -> AudioRoomObbGeometry {
        AudioRoomObbGeometry {
            center: [0.0; 3],
            rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
            half_extents: [5.0, 4.0, 6.0],
        }
    }

    #[test]
    fn second_order_paths_have_two_distinct_bounces_and_are_deterministic() {
        let first = second_order_reflection_geometry(room(), [1.0, 0.5, 0.0], [-1.0, 0.0, 0.5]);
        let second = second_order_reflection_geometry(room(), [1.0, 0.5, 0.0], [-1.0, 0.0, 0.5]);
        assert!(!first.is_empty());
        assert_eq!(first, second);
        assert!(first
            .iter()
            .all(|path| path.face_indices[0] != path.face_indices[1]));
        assert!(first
            .windows(2)
            .all(|pair| { pair[0].path_length_m <= pair[1].path_length_m + 1.0e-6 }));
    }

    #[test]
    fn second_order_bounce_points_lie_on_declared_room_faces() {
        let paths = second_order_reflection_geometry(room(), [0.7, 0.4, -0.8], [-0.6, 0.2, 0.9]);
        assert!(!paths.is_empty());
        let ext = room().half_extents;
        for path in paths {
            for (face, point) in path.face_indices.into_iter().zip(path.reflection_points) {
                let axis = usize::from(face / 2);
                let expected = if face & 1 == 0 { -ext[axis] } else { ext[axis] };
                assert!((point[axis] - expected).abs() < 1.0e-3);
                for component in 0..3 {
                    if component != axis {
                        assert!(point[component].abs() <= ext[component] + 1.0e-3);
                    }
                }
            }
        }
    }

    #[test]
    fn second_order_paths_are_longer_than_direct_path() {
        let source = [1.0, 0.0, 0.0];
        let listener = [-1.0, 0.0, 0.0];
        let direct = length3(sub3(source, listener));
        let paths = second_order_reflection_geometry(room(), source, listener);
        assert!(!paths.is_empty());
        assert!(paths
            .iter()
            .all(|path| path.path_length_m + 1.0e-4 >= direct));
        assert!(paths.iter().any(|path| path.excess_length_m > 0.1));
    }

    #[test]
    fn mesh_diffraction_edges_remove_coplanar_triangulation_diagonal() {
        let vertices = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ];
        let triangles = [[0, 1, 2], [0, 2, 3]];
        let edges = mesh_diffraction_edges(&vertices, &triangles);
        assert_eq!(edges.len(), 4);
        assert!(!edges.iter().any(|edge| edge.vertex_indices == [0, 2]));
        assert!(edges.iter().all(|edge| edge.adjacent_faces == 1));
    }

    #[test]
    fn mesh_diffraction_edges_keep_cube_dihedral_edges_but_not_face_diagonals() {
        let vertices = [
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ];
        let triangles = [
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let edges = mesh_diffraction_edges(&vertices, &triangles);
        assert_eq!(edges.len(), 12);
        assert!(edges.iter().all(|edge| edge.adjacent_faces == 2));
        assert!(edges.iter().all(|edge| {
            (edge.wedge_angle_radians - std::f32::consts::FRAC_PI_2).abs() < 1.0e-4
        }));
    }

    #[test]
    fn finite_edge_diffraction_geometry_finds_symmetric_shortest_detour() {
        let geometry = edge_diffraction_geometry(
            [[-2.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            [-1.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        )
        .expect("edge path");
        assert!(geometry.diffraction_point[0].abs() < 1.0e-3);
        assert!(geometry.path_length_m > 2.8 && geometry.path_length_m < 2.9);
        assert!(geometry.excess_length_m > 0.8);
        assert!(geometry.bend_angle_radians > 1.4 && geometry.bend_angle_radians < 1.7);
    }
