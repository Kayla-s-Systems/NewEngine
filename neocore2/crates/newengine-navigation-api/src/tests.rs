use super::*;

#[test]
fn service_info_exposes_path_planning() {
    let info = NavigationServiceInfoV1::default();
    assert!(info
        .methods
        .iter()
        .any(|method| method == navigation_method::PLAN_PATH_JSON_V1));
}

#[test]
fn nav_vector_constructor_preserves_components() {
    let value = NavVec3::new(1.0, 2.0, 3.0);
    assert_eq!((value.x, value.y, value.z), (1.0, 2.0, 3.0));
}

#[test]
fn path_response_roundtrips_json() {
    let response = NavPlanPathResponseV1 {
        accepted: true,
        path: Some(NavPathDtoV1 {
            complete: true,
            cost: 4.5,
            ..NavPathDtoV1::default()
        }),
        diagnostics: Vec::new(),
    };
    let encoded = serde_json::to_string(&response).unwrap();
    let decoded: NavPlanPathResponseV1 = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, response);
}
