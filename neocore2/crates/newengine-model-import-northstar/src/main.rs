use std::path::PathBuf;

use newengine_model_import_northstar::{
    compile_character, CharacterCompileRequest, SkeletonProfile,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("northstar-importer: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut name = None;
    let mut skeleton = None;
    let mut output_dir = None;
    let mut skeleton_profile = SkeletonProfile::Humanoid;
    let mut material_library_ref = None;
    let mut material_by_source_identity = false;
    let mut packages = Vec::new();
    let mut package_mesh_prefixes = Vec::new();
    let mut material_overrides = Vec::new();
    let mut required_mesh_prefixes = Vec::new();
    let mut package_skin_fallback_joints = Vec::new();
    let mut source_to_model = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--name" => name = args.next(),
            "--skeleton" => skeleton = args.next().map(PathBuf::from),
            "--skeleton-profile" => {
                skeleton_profile = match args.next().as_deref() {
                    Some("humanoid") => SkeletonProfile::Humanoid,
                    Some("generic") | Some("weapon") => SkeletonProfile::Generic,
                    Some(other) => return Err(format!("unsupported --skeleton-profile '{other}'")),
                    None => {
                        return Err("--skeleton-profile requires humanoid|generic|weapon".to_owned())
                    }
                };
            }
            "--package" => packages.push(PathBuf::from(
                args.next().ok_or("--package requires a path")?,
            )),
            "--package-mesh-prefix" => {
                let spec = args
                    .next()
                    .ok_or("--package-mesh-prefix requires PATH::PREFIX")?;
                let (path, prefix) = spec
                    .split_once("::")
                    .ok_or("--package-mesh-prefix requires PATH::PREFIX")?;
                if prefix.trim().is_empty() {
                    return Err("--package-mesh-prefix prefix must not be empty".to_owned());
                }
                package_mesh_prefixes.push((PathBuf::from(path), prefix.to_owned()));
            }
            "--source-to-model" => {
                let raw = args
                    .next()
                    .ok_or("--source-to-model requires 16 comma-separated f32 values")?;
                let values = raw
                    .split(',')
                    .map(str::trim)
                    .map(str::parse::<f32>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| format!("invalid --source-to-model: {error}"))?;
                if values.len() != 16 || values.iter().any(|value| !value.is_finite()) {
                    return Err(
                        "--source-to-model requires exactly 16 finite f32 values".to_owned()
                    );
                }
                let mut matrix = [0.0_f32; 16];
                matrix.copy_from_slice(&values);
                source_to_model = Some(matrix);
            }
            "--material-override" => {
                let spec = args
                    .next()
                    .ok_or("--material-override requires PREFIX=REF")?;
                let (prefix, reference) = spec
                    .split_once('=')
                    .ok_or("--material-override requires PREFIX=REF")?;
                if prefix.trim().is_empty() || reference.trim().is_empty() {
                    return Err("--material-override PREFIX and REF must not be empty".to_owned());
                }
                material_overrides.push((prefix.to_owned(), reference.to_owned()));
            }
            "--require-mesh-prefix" => {
                let prefix = args.next().ok_or("--require-mesh-prefix requires PREFIX")?;
                if prefix.trim().is_empty() {
                    return Err("--require-mesh-prefix must not be empty".to_owned());
                }
                required_mesh_prefixes.push(prefix);
            }
            "--package-skin-fallback" => {
                let spec = args
                    .next()
                    .ok_or("--package-skin-fallback requires PATH::JOINT[,JOINT...]")?;
                let (path, joints) = spec
                    .split_once("::")
                    .ok_or("--package-skin-fallback requires PATH::JOINT[,JOINT...]")?;
                let joints = joints
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                if path.trim().is_empty() || joints.is_empty() {
                    return Err(
                        "--package-skin-fallback PATH and JOINT list must not be empty".to_owned(),
                    );
                }
                package_skin_fallback_joints.push((PathBuf::from(path), joints));
            }
            "--material-by-source-identity" => material_by_source_identity = true,
            "--output-dir" => output_dir = args.next().map(PathBuf::from),
            "--material-library" => material_library_ref = args.next(),
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let request = CharacterCompileRequest {
        name: name.ok_or("--name is required")?,
        package_paths: packages,
        skeleton_path: skeleton.ok_or("--skeleton is required")?,
        skeleton_profile,
        output_dir: output_dir.ok_or("--output-dir is required")?,
        material_library_ref,
        material_by_source_identity,
        package_mesh_prefixes,
        material_overrides,
        required_mesh_prefixes,
        package_skin_fallback_joints,
        source_to_model,
    };
    let report = compile_character(&request)?;
    println!(
        "northstar-importer: PASS name='{}' meshes={} vertices={} indices={} joints={}",
        request.name,
        report.mesh_count,
        report.vertex_count,
        report.index_count,
        report.joint_count
    );
    println!(
        "skin: vertices={} avg_influences={:.3} max_influences={} top4_loss_avg={:.4}% top4_loss_max={:.4}% top8_loss_avg={:.4}% top8_loss_max={:.4}%",
        report.skin_loss.weighted_vertices,
        report.skin_loss.average_source_influences(),
        report.skin_loss.max_source_influences,
        report.skin_loss.average_top4_loss() * 100.0,
        report.skin_loss.top4_loss_max * 100.0,
        report.skin_loss.average_top8_loss() * 100.0,
        report.skin_loss.top8_loss_max * 100.0,
    );
    for (slot, identity) in &report.material_slots {
        println!("material-slot {slot} source='{identity}'");
    }
    for fallback in &report.skin_fallbacks {
        println!(
            "skin-fallback package='{}' mesh='{}' source_domain={} master_joints='{}'",
            fallback.package.display(),
            fallback.mesh,
            fallback.source_joint_domain_size,
            fallback.target_joints.join(",")
        );
    }
    println!("ydd={}", report.ydd_path.display());
    println!("ymt={}", report.ymt_path.display());
    Ok(())
}

fn print_help() {
    println!(
        "Usage: newengine-model-import-northstar --name NAME --skeleton FILE [--skeleton-profile humanoid|weapon] --package FILE [--package FILE...] [--package-mesh-prefix PATH::PREFIX] [--material-override PREFIX=REF] [--require-mesh-prefix PREFIX] [--package-skin-fallback PATH::JOINT[,JOINT...]] [--material-by-source-identity] [--source-to-model M00,...,M33] --output-dir DIR [--material-library REF]"
    );
}
