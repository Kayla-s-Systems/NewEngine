use newengine_model_import_northstar::{
    compile_rigid_joint_variants, RigidJointVariantsCompileRequest,
};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let package_path =
        PathBuf::from(args.next().ok_or(
            "usage: compile_rigid_joint_variants PACKAGE OUTPUT_YDD MATERIAL_REF JOINT...",
        )?);
    let output_path = PathBuf::from(args.next().ok_or("missing OUTPUT_YDD")?);
    let material_ref = args.next().ok_or("missing MATERIAL_REF")?;
    let joints = args.collect::<Vec<_>>();
    let report = compile_rigid_joint_variants(&RigidJointVariantsCompileRequest {
        name: output_path
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("rigid_variants")
            .to_owned(),
        package_path,
        joints,
        output_path,
        material_ref: Some(material_ref),
    })?;
    println!(
        "rigid-joint compile PASS ydd='{}' entries={} meshes={} vertices={} indices={}",
        report.ydd_path.display(),
        report.entry_count,
        report.mesh_count,
        report.vertex_count,
        report.index_count
    );
    Ok(())
}
