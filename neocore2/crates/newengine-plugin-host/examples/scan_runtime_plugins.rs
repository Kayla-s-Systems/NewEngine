use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let dir = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: scan_runtime_plugins <dir>");
    let started = Instant::now();
    match newengine_plugin_host::scan_plugin_discovery_graph(&dir) {
        Ok(graph) => {
            println!(
                "OK elapsed_ms={:.2}",
                started.elapsed().as_secs_f64() * 1000.0
            );
            println!("{graph:#?}");
        }
        Err(error) => {
            eprintln!(
                "ERROR elapsed_ms={:.2} path={} message={}",
                started.elapsed().as_secs_f64() * 1000.0,
                error.path.display(),
                error.message
            );
            std::process::exit(1);
        }
    }
}
