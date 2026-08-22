use std::{env, path::PathBuf, process};
fn main() {
    let root = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: scan_migration_corpus <northstar-root>");
        process::exit(2)
    });
    let rows = newengine_migration_registry::migrations()
        .iter()
        .map(|m| {
            newengine_migration_registry::scan_corpus(&root, m).unwrap_or_else(|e| {
                eprintln!("{}", e.join("\n"));
                process::exit(1)
            })
        })
        .collect::<Vec<_>>();
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}
