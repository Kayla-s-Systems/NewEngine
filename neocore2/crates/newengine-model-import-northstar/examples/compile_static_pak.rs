use newengine_model_import_northstar::{compile_static_pak, StaticPakCompileRequest};
use std::{env, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let package_path = PathBuf::from(args.next().ok_or(
        "usage: compile_static_pak PACKAGE OUTPUT_YDD MATERIAL_REF NAME [--bake-skinned-bind-pose]",
    )?);
    let output_path = PathBuf::from(args.next().ok_or("missing OUTPUT_YDD")?);
    let material_ref = args.next().ok_or("missing MATERIAL_REF")?;
    let name = args.next().ok_or("missing NAME")?;
    let flags = args.collect::<Vec<_>>();
    let bake_skinned_bind_pose = flags.iter().any(|flag| flag == "--bake-skinned-bind-pose");
    let report = compile_static_pak(&StaticPakCompileRequest {
        name,
        package_path,
        output_path,
        material_ref: Some(material_ref),
        bake_skinned_bind_pose,
        source_to_model: None,
    })?;
    println!(
        "static-pak: PASS output='{}' meshes={} vertices={} indices={} bind_pose_baked={}",
        report.ydd_path.display(),
        report.mesh_count,
        report.vertex_count,
        report.index_count,
        report.bind_pose_baked,
    );
    Ok(())
}
