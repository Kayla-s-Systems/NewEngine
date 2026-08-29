use super::*;

#[test]
fn rejects_absolute_and_parent_paths() {
    assert!(normalize_logical_path("C:/tmp/x.obj", false).is_err());
    assert!(normalize_logical_path("../x.obj", false).is_err());
    assert!(normalize_logical_path("/tmp/x.obj", false).is_err());
}

#[test]
fn logical_path_preserves_texture_selector() {
    assert_eq!(
        join_logical_path("models/props", "textures/crate.ytd@albedo").unwrap(),
        "models/props/textures/crate.ytd@albedo"
    );
}

#[test]
fn parses_mtl_material_and_textures() {
    let materials = parse_mtl_text(
        "models/props",
        "newmtl crate\nKd 0.1 0.2 0.3\nd 0.75\nNs 64\nmap_Kd textures/crate.png\nbump textures/crate_n.png",
    );
    let material = materials.get("crate").unwrap();
    assert_eq!(material.kd, [0.1, 0.2, 0.3]);
    assert_eq!(material.alpha, 0.75);
    assert_eq!(material.ns, 64.0);
    assert_eq!(
        material.base_color_texture.as_deref(),
        Some("models/props/textures/crate.png")
    );
}

#[test]
fn decodes_obj_and_loads_mtl() {
    let obj = "mtllib crate.mtl\nusemtl crate\nv 0 0 0\nv 1 0 0\nv 0 2 0\nvt 0 0\nvt 1 0\nvt 0 1\nf 1/1 2/2 3/3";
    let decoded = decode_obj_with_mtl_loader("models/crate.obj", obj, 1.0, |path| {
        (path == "models/crate.mtl").then(|| "newmtl crate\nKd 1 0 0".to_owned())
    })
    .unwrap();

    assert_eq!(decoded.parts.len(), 1);
    assert_eq!(decoded.parts[0].material_slot, "crate");
    assert_eq!(decoded.parts[0].mesh.indices.len(), 3);
    assert!(decoded.materials.contains_key("crate"));
}

#[test]
fn supports_negative_obj_indices() {
    let obj = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1";
    let decoded = decode_obj_with_mtl_loader("models/triangle.obj", obj, 1.0, |_| None).unwrap();
    assert_eq!(decoded.parts[0].mesh.vertices.len(), 3);
}
