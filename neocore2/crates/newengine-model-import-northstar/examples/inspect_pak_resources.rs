use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let bytes = fs::read(&source).map_err(|e| format!("read {source}: {e}"))?;
        let pak = PakFile::parse(bytes)?;
        println!("PAK {source} resources={}", pak.resources().len());
        for (index, resource) in pak.resources().iter().enumerate() {
            println!(
                "{index:03} kind='{}' name='{}' offset=0x{:x}",
                resource.kind, resource.name, resource.absolute_offset
            );
        }
    }
    Ok(())
}
