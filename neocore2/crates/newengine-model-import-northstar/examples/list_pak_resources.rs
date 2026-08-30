use newengine_model_import_northstar::PakFile;
use std::{env, fs};
fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
        println!("PACKAGE {source} resources={}", pak.resources().len());
        for r in pak.resources() {
            println!("{}\t{}\t0x{:x}", r.kind, r.name, r.absolute_offset);
        }
    }
    Ok(())
}
