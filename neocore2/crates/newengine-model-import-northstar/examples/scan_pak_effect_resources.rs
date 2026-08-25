use newengine_model_import_northstar::PakFile;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn collect(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(root) else {
        return;
    };
    for entry in read.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pak")) {
            out.push(p);
        }
    }
}

fn suspicious(s: &str) -> bool {
    let s = s.to_ascii_lowercase();
    [
        "particle",
        "emitter",
        "muzzle",
        "impact",
        "bullet",
        "decal",
        "spark",
        "tracer",
        "projectile",
        "effect",
    ]
    .iter()
    .any(|t| s.contains(t))
}

fn main() -> Result<(), String> {
    let mut files = Vec::new();
    for arg in env::args().skip(1) {
        collect(Path::new(&arg), &mut files);
    }
    files.sort();
    let mut parsed = 0usize;
    let mut failed = 0usize;
    let mut hits = 0usize;
    for file in files {
        let bytes = match fs::read(&file) {
            Ok(v) => v,
            Err(_) => {
                failed += 1;
                continue;
            }
        };
        let pak = match PakFile::parse(bytes) {
            Ok(v) => v,
            Err(_) => {
                failed += 1;
                continue;
            }
        };
        parsed += 1;
        let resource_hits = pak
            .resources()
            .iter()
            .filter(|r| suspicious(&r.kind) || suspicious(&r.name))
            .collect::<Vec<_>>();
        if !resource_hits.is_empty() {
            hits += 1;
            println!(
                "PAK {} bytes={} resources={}",
                file.display(),
                pak.bytes().len(),
                pak.resources().len()
            );
            for r in resource_hits {
                println!(
                    "  kind='{}' name='{}' offset=0x{:x}",
                    r.kind, r.name, r.absolute_offset
                );
            }
        }
    }
    println!("SUMMARY parsed={parsed} failed={failed} hit_paks={hits}");
    Ok(())
}
