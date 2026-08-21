use std::path::PathBuf;

use newengine_model_import_naughtydog::{compile_character, CharacterCompileRequest};

fn main() {
    if let Err(error) = run() {
        eprintln!("naughtydog-importer: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut name = None;
    let mut skeleton = None;
    let mut output_dir = None;
    let mut packages = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--name" => name = args.next(),
            "--skeleton" => skeleton = args.next().map(PathBuf::from),
            "--package" => packages.push(PathBuf::from(
                args.next().ok_or("--package requires a path")?,
            )),
            "--output-dir" => output_dir = args.next().map(PathBuf::from),
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
        output_dir: output_dir.ok_or("--output-dir is required")?,
    };
    let report = compile_character(&request)?;
    println!(
        "naughtydog-importer: PASS name='{}' meshes={} vertices={} indices={} joints={}",
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
        "Usage: newengine-model-import-naughtydog --name NAME --skeleton FILE --package FILE [--package FILE...] --output-dir DIR"
    );
}
