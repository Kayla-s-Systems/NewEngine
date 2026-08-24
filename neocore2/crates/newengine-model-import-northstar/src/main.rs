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
    let mut packages = Vec::new();
    let mut package_mesh_prefixes = Vec::new();
    let mut material_overrides = Vec::new();
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
        package_mesh_prefixes,
        material_overrides,
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
    println!("ydd={}", report.ydd_path.display());
    println!("ymt={}", report.ymt_path.display());
    Ok(())
}

fn print_help() {
    println!(
        "Usage: newengine-model-import-northstar --name NAME --skeleton FILE [--skeleton-profile humanoid|weapon] --package FILE [--package FILE...] [--package-mesh-prefix PATH::PREFIX] [--material-override PREFIX=REF] --output-dir DIR [--material-library REF]"
    );
}
